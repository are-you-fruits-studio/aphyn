use std::collections::HashMap;

use ab_glyph::{Font as _, FontRef, PxScale, ScaleFont as _, VariableFont as _};
use aphyn_asset_storage::{AssetHandle, AssetStorageResource};
use aphyn_ffi::FfiString;
use aphyn_math::Vec2;
use aphyn_reflection::*;
use aphyn_render::{FilterMode, RenderApiResource, StandardTexture};
use aphyn_serialization::*;

// metrics are in pixels of the font's `pixel_size`, y up from the baseline
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct Glyph {
    // texture coordinates of the glyph's top-left and bottom-right corners
    pub uv: [Vec2<f32>; 2],
    // left edge and top edge of the quad relative to the pen position on the baseline
    pub offset: Vec2<f32>,
    pub size: Vec2<f32>,
    pub advance: f32,
}

// todo: ffi?
pub struct Font {
    pub texture: AssetHandle<StandardTexture>,
    pub glyphs: HashMap<char, Glyph>,
    pub missing_glyph: Glyph,
    pub pixel_size: f32,
    pub ascent: f32,
    pub descent: f32,
}

impl Font {
    pub fn glyph(&self, character: char) -> &Glyph {
        self.glyphs.get(&character).unwrap_or(&self.missing_glyph)
    }

    pub fn scale(&self, font_size: f32) -> f32 {
        font_size / self.pixel_size.max(f32::EPSILON)
    }

    pub fn line_height(&self, font_size: f32) -> f32 {
        (self.ascent + self.descent) * self.scale(font_size)
    }

    pub fn measure(&self, text: &str, font_size: f32, horizontal_spacing: f32) -> f32 {
        let scale = self.scale(font_size);
        let mut width = 0.0;
        let mut count = 0;

        for character in text.chars() {
            width += self.glyph(character).advance * scale;
            count += 1;
        }

        width + count.max(1) as f32 * horizontal_spacing - horizontal_spacing
    }

    // the char index whose left edge is closest to `x`, measured from the text's left edge
    pub fn char_index_at(&self, text: &str, font_size: f32, horizontal_spacing: f32, x: f32) -> usize {
        let scale = self.scale(font_size);
        let mut left = 0.0;

        for (i, character) in text.chars().enumerate() {
            let advance = self.glyph(character).advance * scale;
            if x < left + advance * 0.5 {
                return i;
            }
            left += advance + horizontal_spacing;
        }

        text.chars().count()
    }
}

pub struct FontRasterSettings {
    pub pixel_size: f32,
    // the "wght" axis of a variable font; ignored by fonts without it
    pub weight: Option<f32>,
    pub characters: Vec<char>,
}

impl FontRasterSettings {
    pub fn new(pixel_size: f32) -> Self {
        Self {
            pixel_size,
            weight: None,
            characters: default_font_characters(),
        }
    }
}

// printable ASCII, Latin-1, Cyrillic and common typographic punctuation
pub fn default_font_characters() -> Vec<char> {
    let ranges = [
        ' '..='~',
        '\u{a0}'..='\u{ff}',
        '\u{400}'..='\u{4ff}',
        '\u{2010}'..='\u{2027}',
        '\u{2030}'..='\u{203a}',
        '\u{2190}'..='\u{2193}',
        '\u{2713}'..='\u{2717}',
    ];
    ranges.into_iter().flatten().collect()
}

// what a "font" asset file holds: the TTF/OTF payload and how to rasterize it
#[repr(C)]
#[derive(Serializable, Reflectable, Clone, Debug)]
pub struct FontAssetMetadata {
    pub raw_font: FfiString,
    pub size: f32,
    // the "wght" axis of a variable font; 0 keeps the font's default
    pub weight: f32,
    // added to `default_font_characters`
    pub extra_characters: FfiString,
}

impl Default for FontAssetMetadata {
    fn default() -> Self {
        Self {
            raw_font: FfiString::new(),
            size: 16.0,
            weight: 0.0,
            extra_characters: FfiString::new(),
        }
    }
}

impl FontAssetMetadata {
    pub fn raster_settings(&self) -> FontRasterSettings {
        let mut settings = FontRasterSettings::new(self.size);
        settings.weight = (self.weight > 0.0).then_some(self.weight);
        settings.characters.extend(self.extra_characters.as_str().chars());
        settings
    }
}

// the atlas texture goes into `textures`; a text material samples it as `color_tex`
pub fn load_font(
    render_api: &RenderApiResource,
    textures: &mut AssetStorageResource<StandardTexture>,
    data: &[u8],
    settings: &FontRasterSettings,
) -> Result<Font, String> {
    let rasterized = rasterize_font(render_api, data, settings)?;

    Ok(Font {
        texture: textures.insert(rasterized.texture),
        glyphs: rasterized.glyphs,
        missing_glyph: rasterized.missing_glyph,
        pixel_size: settings.pixel_size,
        ascent: rasterized.ascent,
        descent: rasterized.descent,
    })
}

const ATLAS_WIDTH: usize = 512;
const ATLAS_PADDING: usize = 1;

pub struct RasterizedFont {
    pub texture: StandardTexture,
    pub glyphs: HashMap<char, Glyph>,
    pub missing_glyph: Glyph,
    pub ascent: f32,
    pub descent: f32,
}

// glyphs keep their antialiased coverage in the texture's alpha, so text needs an alpha-blended material
pub fn rasterize_font(render_api: &RenderApiResource, data: &[u8], settings: &FontRasterSettings) -> Result<RasterizedFont, String> {
    let mut font = FontRef::try_from_slice(data).map_err(|err| err.to_string())?;

    if let Some(weight) = settings.weight {
        font.set_variation(b"wght", weight);
    }

    let units_per_em = font.units_per_em().ok_or("the font has no units per em")?;
    // a CSS-like font size is the em size, which ab_glyph scales by the font's full height instead
    let px_scale = PxScale::from(settings.pixel_size * font.height_unscaled() / units_per_em);
    let scaled = font.as_scaled(px_scale);

    struct Raster {
        character: char,
        width: usize,
        height: usize,
        coverage: Vec<f32>,
        offset: Vec2<f32>,
        advance: f32,
    }

    let mut rasters = Vec::new();
    for &character in settings.characters.iter().chain(['\u{fffd}', '?'].iter()) {
        let glyph_id = font.glyph_id(character);
        if glyph_id.0 == 0 && character != '?' {
            continue;
        }

        let advance = scaled.h_advance(glyph_id);
        let glyph = glyph_id.with_scale_and_position(px_scale, ab_glyph::point(0.0, 0.0));

        let Some(outlined) = font.outline_glyph(glyph) else {
            rasters.push(Raster {
                character,
                width: 0,
                height: 0,
                coverage: Vec::new(),
                offset: Vec2::splat(0.0),
                advance,
            });
            continue;
        };

        let bounds = outlined.px_bounds();
        let (width, height) = (bounds.width() as usize, bounds.height() as usize);
        let mut coverage = vec![0.0; width * height];
        outlined.draw(|x, y, c| {
            if let Some(v) = coverage.get_mut(y as usize * width + x as usize) {
                *v = c;
            }
        });

        rasters.push(Raster {
            character,
            width,
            height,
            coverage,
            offset: Vec2::new(bounds.min.x, -bounds.min.y),
            advance,
        });
    }

    // rows of glyphs, left to right, each row as tall as its tallest glyph
    let mut positions = Vec::with_capacity(rasters.len());
    let (mut x, mut y, mut row_height) = (ATLAS_PADDING, ATLAS_PADDING, 0);
    for raster in &rasters {
        if x + raster.width + ATLAS_PADDING > ATLAS_WIDTH {
            x = ATLAS_PADDING;
            y += row_height + ATLAS_PADDING;
            row_height = 0;
        }
        positions.push((x, y));
        x += raster.width + ATLAS_PADDING;
        row_height = row_height.max(raster.height);
    }
    let atlas_height = (y + row_height + ATLAS_PADDING).next_power_of_two();

    let mut pixels = vec![0u8; ATLAS_WIDTH * atlas_height * 4];
    for pixel in pixels.chunks_exact_mut(4) {
        pixel[..3].fill(255);
    }

    let mut glyphs = HashMap::new();
    for (raster, &(x, y)) in rasters.iter().zip(&positions) {
        for row in 0..raster.height {
            for column in 0..raster.width {
                let alpha = raster.coverage[row * raster.width + column];
                pixels[((y + row) * ATLAS_WIDTH + x + column) * 4 + 3] = (alpha.clamp(0.0, 1.0) * 255.0).round() as u8;
            }
        }

        let atlas_size = Vec2::new(ATLAS_WIDTH as f32, atlas_height as f32);
        let glyph = Glyph {
            uv: [
                Vec2::new(x as f32, y as f32) / atlas_size,
                Vec2::new((x + raster.width) as f32, (y + raster.height) as f32) / atlas_size,
            ],
            offset: raster.offset,
            size: Vec2::new(raster.width as f32, raster.height as f32),
            advance: raster.advance,
        };
        glyphs.entry(raster.character).or_insert(glyph);
    }

    let missing_glyph = glyphs.get(&'\u{fffd}').or(glyphs.get(&'?')).copied().unwrap_or_default();

    let texture = render_api.create_texture(
        FilterMode::Linear,
        [ATLAS_WIDTH as u32, atlas_height as u32],
        &pixels,
        Default::default(),
    );

    Ok(RasterizedFont {
        texture,
        glyphs,
        missing_glyph,
        ascent: scaled.ascent(),
        descent: -scaled.descent(),
    })
}
