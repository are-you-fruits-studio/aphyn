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
        for character in self.extra_characters.as_str().chars() {
            if !settings.characters.contains(&character) {
                settings.characters.push(character);
            }
        }
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

const ATLAS_MIN_WIDTH: usize = 512;
// wgpu's default max_texture_dimension_2d
pub const FONT_ATLAS_MAX_SIZE: usize = 8192;
const ATLAS_PADDING: usize = 1;

pub struct RasterizedFont {
    pub texture: StandardTexture,
    pub glyphs: HashMap<char, Glyph>,
    pub missing_glyph: Glyph,
    pub ascent: f32,
    pub descent: f32,
}

// RGBA8, white with the glyph coverage in alpha
pub struct FontAtlas {
    pub size: [usize; 2],
    pub pixels: Vec<u8>,
    pub glyphs: HashMap<char, Glyph>,
    pub missing_glyph: Glyph,
    pub ascent: f32,
    pub descent: f32,
}

// glyphs keep their antialiased coverage in the texture's alpha, so text needs an alpha-blended material
pub fn rasterize_font(render_api: &RenderApiResource, data: &[u8], settings: &FontRasterSettings) -> Result<RasterizedFont, String> {
    let atlas = rasterize_font_atlas(data, settings)?;

    let texture = render_api.create_texture(
        FilterMode::Linear,
        atlas.size.map(|v| v as u32),
        &atlas.pixels,
        Default::default(),
    );

    Ok(RasterizedFont {
        texture,
        glyphs: atlas.glyphs,
        missing_glyph: atlas.missing_glyph,
        ascent: atlas.ascent,
        descent: atlas.descent,
    })
}

pub fn rasterize_font_atlas(data: &[u8], settings: &FontRasterSettings) -> Result<FontAtlas, String> {
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

    // rows of glyphs, left to right, each row as tall as its tallest glyph; wide enough for the widest glyph at large sizes
    // about square, so large sizes grow both sides instead of only the height
    let widest = rasters.iter().map(|r| r.width).max().unwrap_or(0);
    let area = rasters
        .iter()
        .map(|r| (r.width + ATLAS_PADDING) * (r.height + ATLAS_PADDING))
        .sum::<usize>();
    let atlas_width = ATLAS_MIN_WIDTH
        .max((widest + ATLAS_PADDING * 2).next_power_of_two())
        .max(area.isqrt().next_power_of_two());

    let mut positions = Vec::with_capacity(rasters.len());
    let (mut x, mut y, mut row_height) = (ATLAS_PADDING, ATLAS_PADDING, 0);
    for raster in &rasters {
        if x + raster.width + ATLAS_PADDING > atlas_width {
            x = ATLAS_PADDING;
            y += row_height + ATLAS_PADDING;
            row_height = 0;
        }
        positions.push((x, y));
        x += raster.width + ATLAS_PADDING;
        row_height = row_height.max(raster.height);
    }
    let atlas_height = (y + row_height + ATLAS_PADDING).next_power_of_two();

    if atlas_width > FONT_ATLAS_MAX_SIZE || atlas_height > FONT_ATLAS_MAX_SIZE {
        return Err(format!(
            "the glyphs need a {atlas_width}x{atlas_height} atlas, above {FONT_ATLAS_MAX_SIZE}; use a smaller size or fewer characters"
        ));
    }

    let mut pixels = vec![0u8; atlas_width * atlas_height * 4];
    for pixel in pixels.chunks_exact_mut(4) {
        pixel[..3].fill(255);
    }

    let mut glyphs = HashMap::new();
    for (raster, &(x, y)) in rasters.iter().zip(&positions) {
        for row in 0..raster.height {
            for column in 0..raster.width {
                let alpha = raster.coverage[row * raster.width + column];
                pixels[((y + row) * atlas_width + x + column) * 4 + 3] = (alpha.clamp(0.0, 1.0) * 255.0).round() as u8;
            }
        }

        let atlas_size = Vec2::new(atlas_width as f32, atlas_height as f32);
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

    Ok(FontAtlas {
        size: [atlas_width, atlas_height],
        pixels,
        glyphs,
        missing_glyph,
        ascent: scaled.ascent(),
        descent: -scaled.descent(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const ONEST: &[u8] = include_bytes!("../../editor/assets/fonts/Onest.ttf");

    fn font_from(atlas: FontAtlas, pixel_size: f32) -> Font {
        Font {
            texture: AssetHandle::EMPTY,
            glyphs: atlas.glyphs,
            missing_glyph: atlas.missing_glyph,
            pixel_size,
            ascent: atlas.ascent,
            descent: atlas.descent,
        }
    }

    fn monospace(advance: f32) -> Font {
        let glyph = Glyph {
            advance,
            ..Default::default()
        };
        Font {
            texture: AssetHandle::EMPTY,
            glyphs: [('a', glyph), ('я', glyph)].into(),
            missing_glyph: glyph,
            pixel_size: 1.0,
            ascent: 1.0,
            descent: 0.0,
        }
    }

    #[test]
    fn measure_adds_spacing_between_chars_only() {
        let font = monospace(0.5);
        assert_eq!(font.measure("", 10.0, 2.0), 0.0);
        assert_eq!(font.measure("a", 10.0, 2.0), 5.0);
        assert_eq!(font.measure("aяa", 10.0, 2.0), 19.0);
    }

    #[test]
    fn char_index_at_rounds_to_the_nearest_edge_by_chars() {
        let font = monospace(1.0);
        assert_eq!(font.char_index_at("яяя", 10.0, 0.0, -5.0), 0);
        assert_eq!(font.char_index_at("яяя", 10.0, 0.0, 4.0), 0);
        assert_eq!(font.char_index_at("яяя", 10.0, 0.0, 6.0), 1);
        assert_eq!(font.char_index_at("яяя", 10.0, 0.0, 1000.0), 3);
        assert_eq!(font.char_index_at("", 10.0, 0.0, 3.0), 0);
    }

    #[test]
    fn atlas_covers_latin_and_cyrillic_with_glyphs_inside_the_texture() {
        let atlas = rasterize_font_atlas(ONEST, &FontRasterSettings::new(12.0)).unwrap();
        let [width, height] = atlas.size;
        assert!(height.is_power_of_two());
        assert_eq!(atlas.pixels.len(), width * height * 4);

        for c in ['A', 'g', 'Ж', 'ї', 'Ґ', '·', '…'] {
            let glyph = atlas.glyphs.get(&c).unwrap_or_else(|| panic!("no glyph for {c:?}"));
            assert!(glyph.advance > 0.0, "{c:?}");
            assert!(glyph.size.x > 0.0 && glyph.size.y > 0.0, "{c:?}");
            for uv in glyph.uv {
                assert!((0.0..=1.0).contains(&uv.x) && (0.0..=1.0).contains(&uv.y), "{c:?}");
            }
            let px = (glyph.uv[1] - glyph.uv[0]) * Vec2::new(width as f32, height as f32);
            assert!(
                (px - glyph.size).map(f32::abs).into_array().iter().all(|d| *d < 1e-3),
                "{c:?}"
            );
        }

        let space = atlas.glyphs[&' '];
        assert_eq!(space.size, Vec2::splat(0.0));
        assert!(space.advance > 0.0);
    }

    #[test]
    fn glyphs_sit_on_the_baseline() {
        let atlas = rasterize_font_atlas(ONEST, &FontRasterSettings::new(12.0)).unwrap();
        let x = atlas.glyphs[&'x'];
        let p = atlas.glyphs[&'p'];
        // a lowercase x rests on the baseline, a p descends below it
        assert!(x.offset.y > 0.0 && (x.offset.y - x.size.y).abs() <= 1.0);
        assert!(p.offset.y - p.size.y < -1.0);
        assert!(atlas.ascent > 0.0 && atlas.descent > 0.0);
    }

    #[test]
    fn glyph_rects_dont_overlap() {
        let atlas = rasterize_font_atlas(ONEST, &FontRasterSettings::new(14.0)).unwrap();
        let size = Vec2::new(atlas.size[0] as f32, atlas.size[1] as f32);
        let mut rects = atlas
            .glyphs
            .values()
            .filter(|g| g.size.x > 0.0)
            .map(|g| (g.uv[0] * size, g.uv[1] * size))
            .collect::<Vec<_>>();
        rects.sort_by(|a, b| (a.0.y, a.0.x).partial_cmp(&(b.0.y, b.0.x)).unwrap());
        rects.dedup();

        for (i, a) in rects.iter().enumerate() {
            for b in &rects[i + 1..] {
                let overlaps = a.0.x < b.1.x && b.0.x < a.1.x && a.0.y < b.1.y && b.0.y < a.1.y;
                assert!(!overlaps, "{a:?} overlaps {b:?}");
            }
        }
    }

    #[test]
    fn huge_sizes_widen_the_atlas() {
        let mut settings = FontRasterSettings::new(700.0);
        settings.characters = vec!['W', 'Ж'];
        let atlas = rasterize_font_atlas(ONEST, &settings).unwrap();
        let widest = atlas.glyphs.values().map(|g| g.size.x as usize).max().unwrap();
        assert!(widest > 512);
        assert!(atlas.size[0] >= widest + 2);
    }

    #[test]
    fn large_sizes_stay_within_the_texture_limit() {
        for size in [64.0, 150.0, 300.0] {
            let atlas = rasterize_font_atlas(ONEST, &FontRasterSettings::new(size)).unwrap();
            assert!(
                atlas.size.iter().all(|&v| v <= FONT_ATLAS_MAX_SIZE),
                "{size}: {:?}",
                atlas.size
            );
        }
    }

    #[test]
    fn too_large_atlas_is_an_error() {
        assert!(rasterize_font_atlas(ONEST, &FontRasterSettings::new(1000.0)).is_err());
    }

    #[test]
    fn extra_characters_are_not_rasterized_twice() {
        let meta = FontAssetMetadata {
            extra_characters: "aaЖ\u{25b8}".into(),
            ..Default::default()
        };
        let characters = meta.raster_settings().characters;
        assert_eq!(characters.iter().filter(|&&c| c == 'a').count(), 1);
        assert_eq!(characters.iter().filter(|&&c| c == 'Ж').count(), 1);
        assert!(characters.contains(&'\u{25b8}'));
    }

    #[test]
    fn heavier_weight_widens_glyphs_of_a_variable_font() {
        let mut light = FontRasterSettings::new(32.0);
        light.weight = Some(300.0);
        let mut bold = FontRasterSettings::new(32.0);
        bold.weight = Some(800.0);
        let light = font_from(rasterize_font_atlas(ONEST, &light).unwrap(), 32.0);
        let bold = font_from(rasterize_font_atlas(ONEST, &bold).unwrap(), 32.0);
        assert!(bold.measure("Weight", 32.0, 0.0) > light.measure("Weight", 32.0, 0.0));
    }

    #[test]
    fn unknown_characters_use_the_missing_glyph() {
        let atlas = rasterize_font_atlas(ONEST, &FontRasterSettings::new(12.0)).unwrap();
        let font = font_from(atlas, 12.0);
        assert_eq!(font.glyph('\u{4e00}'), &font.missing_glyph);
        assert!(font.missing_glyph.advance > 0.0);
    }

    #[test]
    fn invalid_font_data_is_an_error() {
        assert!(rasterize_font_atlas(b"not a font", &FontRasterSettings::new(12.0)).is_err());
    }
}
