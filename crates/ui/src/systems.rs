use core::f32;
use std::collections::HashMap;

use aphyn_asset_storage::{AssetHandle, AssetStorageResource};
use aphyn_ecs::*;
use aphyn_math::*;
use aphyn_render_features::BatchedMeshComponent;
use aphyn_render::*;

use aphyn_transform::*;
use aphyn_window::WindowResource;
use image::GenericImageView;

use crate::{
    ChildrenRectMaskComponent, Font, HorizontalAlign, ImageComponent, ImageFillSettings, StandardUiAssetsResource, TextComponent,
    UiLineComponent, VerticalAlign,
};

pub fn create_standard_ui_assets_resource(mut world: WorldDataMut) {
    let render_api = world.as_ref().resources().get::<RenderApiResource>().unwrap();

    let texture_white = render_api.create_texture(FilterMode::Linear, [2, 2], &[255; 16], Default::default());

    let texture_white = world.as_mut()
        .resources_mut()
        .get_mut::<AssetStorageResource<StandardTexture>>()
        .unwrap()
        .insert(texture_white);

    let (texture_text_px_5_7, font_px_5_7) =
        create_ascii_monospace_font(world.as_mut(), include_bytes!("./assets/ascii_px_5x7.png"));
    let (texture_text_px_8_8, font_px_8_8) =
        create_ascii_monospace_font(world.as_mut(), include_bytes!("./assets/ascii_px_8x8.png"));
    let (texture_text_px_8_12, font_px_8_12) =
        create_ascii_monospace_font(world.as_mut(), include_bytes!("./assets/ascii_px_8x12.png"));

    world
        .resources_mut()
        .insert(StandardUiAssetsResource {
            texture_white,
            texture_text_px_5_7,
            font_px_5_7,
            texture_text_px_8_8,
            font_px_8_8,
            texture_text_px_8_12,
            font_px_8_12,
        });
}

fn create_ascii_monospace_font(mut world: WorldDataMut, texture_bytes: &[u8]) -> (AssetHandle<StandardTexture>, AssetHandle<Font>) {
    // todo: shouldn't this be in an ui module?

    let image = image::load_from_memory(texture_bytes).unwrap();

    let texture_dimensions: [u32; 2] = image.dimensions().into();

    let render_api = world.as_ref().resources().get::<RenderApiResource>().unwrap();

    let texture = render_api.create_texture(FilterMode::Nearest, texture_dimensions, image.as_bytes(), Default::default());

    let text_chars_count = [16, 8];
    let single_char_uv_size = [1.0 / text_chars_count[0] as f32, 1.0 / text_chars_count[1] as f32];

    let characters_uv = (' '..='~')
        .map(|c| {
            let char_uv_index = [c as i32 % text_chars_count[0], c as i32 / text_chars_count[0]];
            let char_uv_min = aphyn_math::zip(&char_uv_index, &text_chars_count, |a, b| *a as f32 / *b as f32);

            let char_uvs = [
                Vec2::from_array(char_uv_min),
                Vec2::from_array(aphyn_math::zip(&char_uv_min, &single_char_uv_size, |a, b| a + b)),
            ];

            (c, char_uvs)
        })
        .collect::<HashMap<_, _>>();

    let texture = world.as_mut()
        .resources_mut()
        .get_mut::<AssetStorageResource<StandardTexture>>()
        .unwrap()
        .insert(texture);

    let font = Font {
        texture: texture.clone(),
        missing_character_uv: characters_uv[&'?'],
        characters_uv,
        character_ratio: (text_chars_count[1] as f32 / text_chars_count[0] as f32)
            * (texture_dimensions[0] as f32 / texture_dimensions[1] as f32),
    };

    let font = world
        .resources_mut()
        .get_mut::<AssetStorageResource<Font>>()
        .unwrap()
        .insert(font);

    (texture, font)
}

pub fn update_image_batched_mesh(mut q: WorldQuery<(&ImageComponent, &mut BatchedMeshComponent, Option<&GlobalRectComponent>)>) {
    for (image_c, mesh_c, rect_c) in q.iter_mut() {
        let color = image_c.color.into_array();

        let create_vertex = |uv, position| StandardVertex {
            color,
            normal: [0.0, 0.0, -1.0],
            tangent: [1.0, 0.0, 0.0, 1.0],
            uv,
            position,
        };

        let mut rect = rect_c.copied().unwrap_or(GlobalRectComponent {
            center: Vec2::splat(0.5),
            scale: Vec2::splat(1.0),
            z: 0.0,
        });

        if image_c.is_y_inverted {
            rect.scale.y *= -1.0;
        }

        let center = rect.center;

        let pos = [center - rect.scale * 0.5, center, center + rect.scale * 0.5];

        let fill_amt = image_c.fill_amt.clamp(0.0, 1.0);

        let clear_fill_f = |mesh_c: &mut BatchedMeshComponent| {
            mesh_c.vertices.clear();
            mesh_c.indices.clear();
        };

        let standard_fill_f = |mesh_c: &mut BatchedMeshComponent| {
            mesh_c.vertices.resize(4, StandardVertex::default());
            mesh_c.indices.resize(6, 0);

            mesh_c.vertices[0] = create_vertex([0.0, 0.0], [pos[0][0], pos[2][1], rect.z]);
            mesh_c.vertices[1] = create_vertex([1.0, 0.0], [pos[2][0], pos[2][1], rect.z]);
            mesh_c.vertices[2] = create_vertex([0.0, 1.0], [pos[0][0], pos[0][1], rect.z]);
            mesh_c.vertices[3] = create_vertex([1.0, 1.0], [pos[2][0], pos[0][1], rect.z]);

            mesh_c.indices[0] = 0;
            mesh_c.indices[1] = 3;
            mesh_c.indices[2] = 1;
            mesh_c.indices[3] = 0;
            mesh_c.indices[4] = 2;
            mesh_c.indices[5] = 3;
        };

        match &image_c.fill_settings {
            _ if fill_amt == 1.0 => standard_fill_f(mesh_c),
            _ if fill_amt == 0.0 => clear_fill_f(mesh_c),
            ImageFillSettings::RadialCenter => {
                let uvs = [
                    [0.5, 0.5],
                    [0.5, 1.0],
                    [0.0, 1.0],
                    [0.0, 0.5],
                    [0.0, 0.0],
                    [0.5, 0.0],
                    [1.0, 0.0],
                    [1.0, 0.5],
                    [1.0, 1.0],
                    [0.5, 1.0],
                ];

                let poss = [
                    [pos[1][0], pos[1][1], rect.z],
                    [pos[1][0], pos[2][1], rect.z],
                    [pos[0][0], pos[2][1], rect.z],
                    [pos[0][0], pos[1][1], rect.z],
                    [pos[0][0], pos[0][1], rect.z],
                    [pos[1][0], pos[0][1], rect.z],
                    [pos[2][0], pos[0][1], rect.z],
                    [pos[2][0], pos[1][1], rect.z],
                    [pos[2][0], pos[2][1], rect.z],
                ];

                let fill_amt = image_c.fill_amt.clamp(0.0, 1.0);

                let slices = 1 + ((fill_amt * 8.0).floor() as u64).clamp(0, 7);

                mesh_c.vertices.resize(3 + slices, StandardVertex::default());
                mesh_c.indices.resize(slices * 3, 0);

                mesh_c.vertices[0] = create_vertex(uvs[0], poss[0]);
                mesh_c.vertices[1] = create_vertex(uvs[1], poss[1]);

                for i in 0..slices {
                    if i + 1 == slices {
                        let (x, y) = (fill_amt * 2.0 * f32::consts::PI).sin_cos();

                        let t = Vec2::new(x, -y) / f32::max(x.abs(), y.abs());

                        let last_pos = pos[1].lerp_separately(pos[0], t);

                        mesh_c.vertices[i + 2] = create_vertex(uvs[i as usize + 2], [last_pos[0], last_pos[1], rect.z]);
                    } else {
                        mesh_c.vertices[i + 2] = create_vertex(uvs[i as usize + 2], poss[i as usize + 2]);
                    }

                    mesh_c.indices[i * 3 + 0] = (i + 1) as u32;
                    mesh_c.indices[i * 3 + 1] = (i + 2) as u32;
                    mesh_c.indices[i * 3 + 2] = 0;
                }
            }
        }
    }
}

pub fn update_text_batched_mesh(
    mut q: WorldQuery<(&TextComponent, &mut BatchedMeshComponent, Option<&GlobalRectComponent>)>,
    window: Res<WindowResource>,
    font_assets: Res<AssetStorageResource<Font>>,
) {
    const VERTICES_PER_CHAR: u64 = 4;
    const INDICES_PER_CHAR: u64 = 6;

    let window_size = window.state().size;
    let window_size = Vec2::from_array(window_size.map(|v| v as f32));

    for (text_c, mesh_c, rect_c) in q.iter_mut() {
        let color = text_c.color.into_array();
        let font = font_assets.get(&text_c.font).unwrap();

        let create_vertex = |uv, position| StandardVertex {
            color,
            normal: [0.0, 0.0, -1.0],
            tangent: [1.0, 0.0, 0.0, 1.0],
            uv,
            position,
        };

        let rect = rect_c.copied().unwrap_or(GlobalRectComponent {
            center: Vec2::splat(0.0),
            scale: Vec2::splat(0.0),
            z: 0.0,
        });

        let ui_val_to_px_fn = |ui_val: UiVal| ui_val.into_px(rect_c.map(|r| r.scale).unwrap_or(window_size), window_size);

        let font_size = ui_val_to_px_fn(text_c.font_size)[1];
        let horizontal_spacing = ui_val_to_px_fn(text_c.horizontal_spacing)[1];

        let mut quad_scale = Vec2::new(font_size * font.character_ratio, font_size);

        let chars_count = text_c.text.chars().count() as u64;

        let mut text_scale = quad_scale;
        text_scale.x *= chars_count as f32;
        text_scale.x += (u64::max(chars_count, 1) - 1) as f32 * horizontal_spacing;

        let center = Vec2::new(
            match text_c.horizontal_align {
                HorizontalAlign::Left => rect.center.x - rect.scale.x * 0.5 + text_scale.x * 0.5,
                HorizontalAlign::Middle => rect.center.x,
                HorizontalAlign::Right => rect.center.x + rect.scale.x * 0.5 - text_scale.x * 0.5,
            },
            match text_c.vertical_align {
                VerticalAlign::Top => rect.center.y - rect.scale.y * 0.5 + text_scale.y * 0.5,
                VerticalAlign::Middle => rect.center.y,
                VerticalAlign::Bottom => rect.center.y + rect.scale.y * 0.5 - text_scale.y * 0.5,
            },
        );

        if text_c.is_y_inverted {
            quad_scale.y *= -1.0;
            text_scale.y *= -1.0;
        }

        let start_pos = center - text_scale * 0.5;

        mesh_c
            .vertices
            .resize((chars_count * VERTICES_PER_CHAR) as u64, StandardVertex::default());
        mesh_c.indices.resize((chars_count * INDICES_PER_CHAR) as u64, 0);

        for (i, character) in text_c.text.chars().enumerate() {
            let i = i as u64;
            let char_uvs = font.characters_uv.get(&character).unwrap_or(&font.missing_character_uv);

            let pos = [
                start_pos + Vec2::new((i + 0) as f32, 0.0) * quad_scale + Vec2::X * horizontal_spacing * i as f32,
                start_pos + Vec2::new((i + 1) as f32, 1.0) * quad_scale + Vec2::X * horizontal_spacing * i as f32,
            ];

            mesh_c.vertices[i * VERTICES_PER_CHAR + 0] = create_vertex([char_uvs[0][0], char_uvs[0][1]], [pos[0][0], pos[1][1], rect.z]);
            mesh_c.vertices[i * VERTICES_PER_CHAR + 1] = create_vertex([char_uvs[1][0], char_uvs[0][1]], [pos[1][0], pos[1][1], rect.z]);
            mesh_c.vertices[i * VERTICES_PER_CHAR + 2] = create_vertex([char_uvs[0][0], char_uvs[1][1]], [pos[0][0], pos[0][1], rect.z]);
            mesh_c.vertices[i * VERTICES_PER_CHAR + 3] = create_vertex([char_uvs[1][0], char_uvs[1][1]], [pos[1][0], pos[0][1], rect.z]);

            mesh_c.indices[i * INDICES_PER_CHAR + 0] = (i * VERTICES_PER_CHAR + 0) as u32;
            mesh_c.indices[i * INDICES_PER_CHAR + 1] = (i * VERTICES_PER_CHAR + 3) as u32;
            mesh_c.indices[i * INDICES_PER_CHAR + 2] = (i * VERTICES_PER_CHAR + 1) as u32;
            mesh_c.indices[i * INDICES_PER_CHAR + 3] = (i * VERTICES_PER_CHAR + 0) as u32;
            mesh_c.indices[i * INDICES_PER_CHAR + 4] = (i * VERTICES_PER_CHAR + 2) as u32;
            mesh_c.indices[i * INDICES_PER_CHAR + 5] = (i * VERTICES_PER_CHAR + 3) as u32;
        }
    }
}

pub fn update_line_batched_mesh(mut q: WorldQuery<(&UiLineComponent, &mut BatchedMeshComponent, &GlobalRectComponent)>) {
    for (line_c, mesh_c, rect_c) in q.iter_mut() {
        mesh_c.vertices.clear();
        mesh_c.indices.clear();

        let color = line_c.color.into_array();
        let half = line_c.thickness_px * 0.5;
        let min = rect_c.center - rect_c.scale * 0.5;
        let to_global = |p: Vec2<f32>| min + p * rect_c.scale;

        let create_vertex = |position: Vec2<f32>| StandardVertex {
            color,
            normal: [0.0, 0.0, -1.0],
            tangent: [1.0, 0.0, 0.0, 1.0],
            uv: [0.5, 0.5],
            position: [position.x, position.y, rect_c.z],
        };

        let points = line_c.points.as_slice();
        let segments: Box<dyn Iterator<Item = &[Vec2<f32>]>> = if line_c.is_segments {
            Box::new(points.chunks_exact(2))
        } else {
            Box::new(points.windows(2))
        };

        for segment in segments {
            let (a, b) = (to_global(segment[0]), to_global(segment[1]));
            let dir = b - a;
            let len = (dir.x * dir.x + dir.y * dir.y).sqrt();
            if len < f32::EPSILON {
                continue;
            }

            // extending each end by half the thickness closes the gaps at polyline joints
            let dir = dir / len * half;
            let normal = Vec2::new(-dir.y, dir.x);
            let (a, b) = (a - dir, b + dir);

            let base = mesh_c.vertices.len() as u32;
            for p in [a + normal, b + normal, a - normal, b - normal] {
                mesh_c.vertices.push(create_vertex(p));
            }
            // both windings: the ui templates cull back faces
            for i in [0, 3, 1, 0, 2, 3, 0, 1, 3, 0, 3, 2] {
                mesh_c.indices.push(base + i);
            }
        }
    }
}

pub fn update_masked_batched_mesh(
    hierarchy_q: WorldQuery<(EntityId, Option<&ChildComponent>, Option<&ParentComponent>)>,
    mask_q: WorldQuery<(EntityId, &GlobalRectComponent), WithFilter<ChildrenRectMaskComponent>>,
    mut mesh_q: WorldQuery<&mut BatchedMeshComponent>,
) {
    for (e, (min, max)) in collect_mask_clip_rects(&hierarchy_q, &mask_q) {
        if let Some(mesh) = mesh_q.get_mut(e) {
            crop_mesh_to_rect(mesh, min, max);
        }
    }
}

// the (min, max) rect each entity at or below a `ChildrenRectMaskComponent` is clipped to:
// the intersection of the rects of every mask among it and its ancestors
pub fn collect_mask_clip_rects(
    hierarchy_q: &WorldQuery<(EntityId, Option<&ChildComponent>, Option<&ParentComponent>)>,
    mask_q: &WorldQuery<(EntityId, &GlobalRectComponent), WithFilter<ChildrenRectMaskComponent>>,
) -> HashMap<EntityId, (Vec2<f32>, Vec2<f32>)> {
    let mut roots = Vec::new();

    for (mask_e, _) in mask_q.iter() {
        let mut top = mask_e;
        let mut current = mask_e;

        while let Some((_, Some(child_c), _)) = hierarchy_q.get(current) {
            current = child_c.parent();

            if hierarchy_q.get(current).is_none() {
                break;
            }

            if mask_q.get(current).is_some() {
                top = current;
            }
        }

        if !roots.contains(&top) {
            roots.push(top);
        }
    }

    let mut clip_rects = HashMap::new();

    for root in roots {
        aphyn_transform::hierarchy_iter_depth_first_parent_to_child(hierarchy_q, Some(root), |e, _, parent_clip| {
            let own = mask_q.get(e).map(|(_, r)| (r.center - r.scale * 0.5, r.center + r.scale * 0.5));

            let clip = match (parent_clip.copied(), own) {
                (None, None) => return None,
                (Some(clip), None) | (None, Some(clip)) => clip,
                (Some((p_min, p_max)), Some((c_min, c_max))) => {
                    (p_min.zip_copied(c_min, f32::max), p_max.zip_copied(c_max, f32::min))
                }
            };

            clip_rects.insert(e, clip);
            Some(clip)
        });
    }

    clip_rects
}

// clamps positions into the rect and moves uvs along through each triangle's position-to-uv mapping:
// exact for axis-aligned quads (images, glyphs), approximate for other triangles
fn crop_mesh_to_rect(mesh: &mut BatchedMeshComponent, min: Vec2<f32>, max: Vec2<f32>) {
    let original = mesh.vertices.as_slice().to_vec();
    let mut is_cropped = vec![false; original.len()];

    let pos_of = |v: &StandardVertex| Vec3::from_array(v.position).xy();
    let uv_of = |v: &StandardVertex| Vec2::from_array(v.uv);
    let clamp = |pos: Vec2<f32>| pos.zip_copied(min, f32::max).zip_copied(max, f32::min);

    for triangle in mesh.indices.as_slice().chunks_exact(3) {
        let [a, b, c] = [triangle[0], triangle[1], triangle[2]].map(|i| i as usize);
        let (Some(va), Some(vb), Some(vc)) = (original.get(a), original.get(b), original.get(c)) else {
            continue;
        };

        let (pa, e1, e2) = (pos_of(va), pos_of(vb) - pos_of(va), pos_of(vc) - pos_of(va));
        let det = e1.x * e2.y - e1.y * e2.x;
        if det.abs() < f32::EPSILON {
            continue;
        }

        for i in [a, b, c] {
            if is_cropped[i] {
                continue;
            }
            is_cropped[i] = true;

            let pos = pos_of(&original[i]);
            let clamped = clamp(pos);
            if clamped == pos {
                continue;
            }

            let d = clamped - pa;
            let s = (d.x * e2.y - d.y * e2.x) / det;
            let t = (e1.x * d.y - e1.y * d.x) / det;
            let uv = uv_of(va) + (uv_of(vb) - uv_of(va)) * s + (uv_of(vc) - uv_of(va)) * t;

            let vertex = &mut mesh.vertices[i as u64];
            vertex.position = [clamped.x, clamped.y, vertex.position[2]];
            vertex.uv = uv.into_array();
        }
    }

    for (i, vertex) in mesh.vertices.iter_mut().enumerate() {
        if !is_cropped[i] {
            let pos = clamp(Vec3::from_array(vertex.position).xy());
            vertex.position = [pos.x, pos.y, vertex.position[2]];
        }
    }
}
