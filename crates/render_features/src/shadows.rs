use aphyn_ecs::Resource;
use aphyn_ffi::FfiVec;
use aphyn_math::{Mat3, Mat4, Vec3, Vec4};
use aphyn_render::{Camera, SHADOW_VIEWS_MAX, StandardLight, StandardShadowView, write_buffer_elements_with};
use aphyn_render_core::{
    GpuCompareFunction, GpuFilterMode, GpuSampler, GpuTexture, GpuTextureFormat, GpuUniformStorageBuffer, GpuViewport, RenderApi,
};

use crate::ShadowSettingsResource;

const SPOT_FOV_MAX: f32 = 170.0 * std::f32::consts::PI / 180.0;

#[repr(C)]
#[derive(Resource)]
pub struct ShadowAtlasResource {
    texture: GpuTexture,
    sampler: GpuSampler,
    views: FfiVec<StandardShadowView>,
    caster_uniforms: FfiVec<GpuUniformStorageBuffer>,
}

impl ShadowAtlasResource {
    pub(crate) fn new(core: &RenderApi) -> Self {
        Self {
            texture: create_atlas_texture(core, 1),
            sampler: core.create_comparison_sampler(
                Some("shadow atlas sampler"),
                GpuFilterMode::Linear,
                GpuCompareFunction::GreaterEqual,
            ),
            views: FfiVec::new(),
            caster_uniforms: FfiVec::new(),
        }
    }

    pub fn texture(&self) -> &GpuTexture {
        &self.texture
    }

    pub fn sampler(&self) -> &GpuSampler {
        &self.sampler
    }

    pub fn views(&self) -> &[StandardShadowView] {
        &self.views
    }

    pub fn caster_uniform(&self, view_index: usize) -> &GpuUniformStorageBuffer {
        &self.caster_uniforms[view_index as u64]
    }

    pub fn viewport(&self, view_index: usize) -> GpuViewport {
        let rect = self.views[view_index as u64].atlas_rect;
        let size = self.texture.size()[0] as f32;

        GpuViewport {
            x: rect.x * size,
            y: rect.y * size,
            width: rect.z * size,
            height: rect.w * size,
            min_depth: 0.0,
            max_depth: 1.0,
        }
    }

    pub(crate) fn views_mut(&mut self) -> &mut FfiVec<StandardShadowView> {
        &mut self.views
    }

    // grows the atlas to `atlas_size` only when shadows are actually rendered
    pub(crate) fn prepare(&mut self, core: &RenderApi, atlas_size: u32) {
        if self.views.is_empty() {
            return;
        }

        if self.texture.size()[0] != atlas_size as usize {
            self.texture = create_atlas_texture(core, atlas_size as usize);
        }

        while self.caster_uniforms.len() < self.views.len() {
            self.caster_uniforms
                .push(core.create_uniform_storage_buffer(Some("shadow caster uniform"), size_of::<Mat4<f32>>()));
        }

        for (view, uniform) in self.views.iter().zip(self.caster_uniforms.iter()) {
            write_buffer_elements_with(core, uniform, 0, 1, |_| view.world_to_clip);
        }
    }
}

fn create_atlas_texture(core: &RenderApi, size: usize) -> GpuTexture {
    core.create_texture(Some("shadow atlas"), [size, size], GpuTextureFormat::Depth32Float, 1)
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct ShadowCamera {
    pub world_to_clip: Mat4<f32>,
    pub camera: Camera,
}

pub(crate) fn tiles_count(settings: &ShadowSettingsResource) -> usize {
    let per_row = tiles_per_row(settings) as usize;

    (per_row * per_row).min(SHADOW_VIEWS_MAX)
}

fn tiles_per_row(settings: &ShadowSettingsResource) -> u32 {
    (settings.atlas_size / settings.tile_size.max(1)).max(1)
}

pub(crate) fn tile_rect(settings: &ShadowSettingsResource, tile: usize) -> Vec4<f32> {
    let per_row = tiles_per_row(settings) as usize;
    let scale = 1.0 / per_row as f32;

    Vec4::new((tile % per_row) as f32 * scale, (tile / per_row) as f32 * scale, scale, scale)
}

pub(crate) fn cascade_splits(near: f32, far: f32, count: usize, lambda: f32) -> impl Iterator<Item = f32> {
    let near = near.max(0.001);
    let far = far.max(near);

    (1..=count).map(move |i| {
        let t = i as f32 / count as f32;
        let uniform = near + (far - near) * t;
        let log = near * (far / near).powf(t);

        uniform + (log - uniform) * lambda
    })
}

// rotation with `forward` as its local +z, the direction cameras look along
pub(crate) fn look_rotation(forward: Vec3<f32>) -> Mat3<f32> {
    let up_hint = match forward.y.abs() > 0.99 {
        true => Vec3::new(0.0, 0.0, 1.0),
        false => Vec3::new(0.0, 1.0, 0.0),
    };
    let right = up_hint.cross(forward).normalized();
    let up = forward.cross(right);

    Mat3::from_array([right.into_array(), up.into_array(), forward.into_array()])
}

fn world_to_view(rotation: Mat3<f32>, eye: Vec3<f32>) -> Mat4<f32> {
    rotation.into_4x4_with_offset(eye).inverse().unwrap_or(Mat4::IDENTITY)
}

fn spot_view(settings: &ShadowSettingsResource, center: Vec3<f32>, direction: Vec3<f32>, range: f32, fov: f32) -> Mat4<f32> {
    let near = settings.spot_near.max(0.001);
    let projection = aphyn_math::perspective_proj_matrix(fov.clamp(0.01, SPOT_FOV_MAX), near, range.max(near * 2.0), 1.0);

    projection * world_to_view(look_rotation(direction), center)
}

pub(crate) fn cascade_view(
    settings: &ShadowSettingsResource,
    camera: &ShadowCamera,
    direction: Vec3<f32>,
    split_near: f32,
    split_far: f32,
) -> Mat4<f32> {
    let clip_to_world = camera.world_to_clip.inverse().unwrap_or(Mat4::IDENTITY);
    let (near, far) = (camera.camera.near, camera.camera.far);
    let depth_t = |depth: f32| (depth - near) / (far - near);
    let (t_near, t_far) = (depth_t(split_near), depth_t(split_far));

    let mut corners = [Vec3::splat(0.0); 8];
    let mut corner_index = 0;

    for x in [-1.0, 1.0] {
        for y in [-1.0, 1.0] {
            let corner_near = clip_to_world.mul_with_projection(Vec3::new(x, y, 1.0));
            let corner_far = clip_to_world.mul_with_projection(Vec3::new(x, y, 0.0));

            for t in [t_near, t_far] {
                corners[corner_index] = corner_near + (corner_far - corner_near) * t;
                corner_index += 1;
            }
        }
    }

    let center = corners.iter().fold(Vec3::splat(0.0), |sum, corner| sum + *corner) * (1.0 / 8.0);
    let radius = corners
        .iter()
        .map(|corner| (*corner - center).length() as f32)
        .fold(0.0, f32::max);
    // a constant size per cascade keeps the texel grid fixed while the camera rotates
    let radius = ((radius * 16.0).ceil() / 16.0).max(0.01);

    let rotation = look_rotation(direction);
    let [right, up, forward] = rotation.as_array().map(Vec3::from_array);

    let texel = 2.0 * radius / settings.tile_size.max(1) as f32;
    let snap = |v: f32| (v / texel).floor() * texel;
    let center = right * snap(center.dot(right)) + up * snap(center.dot(up)) + forward * center.dot(forward);

    let back = radius + settings.caster_distance.max(0.0);
    let eye = center - forward * back;
    let projection = aphyn_math::orthographic_proj_matrix(2.0 * radius, 0.0, back + radius, 1.0);

    projection * world_to_view(rotation, eye)
}

// appends the light's shadow views, returns (start, count); (0, 0) if the light has no shadows
pub(crate) fn push_light_shadow_views(
    settings: &ShadowSettingsResource,
    camera: &ShadowCamera,
    light: &StandardLight,
    views: &mut FfiVec<StandardShadowView>,
) -> Result<(u32, u32), ()> {
    if !light.shadows() {
        return Ok((0, 0));
    }

    let start = views.len() as usize;
    let free = tiles_count(settings).saturating_sub(start);

    let mut push = |world_to_clip, split_far| {
        let atlas_rect = tile_rect(settings, views.len() as usize);
        views.push(StandardShadowView {
            world_to_clip,
            atlas_rect,
            split_far,
            ..Default::default()
        });
    };

    match *light {
        // todo: point light shadows
        StandardLight::Point { .. } => return Ok((0, 0)),
        StandardLight::Spot {
            center,
            range,
            direction_dst,
            fov,
            ..
        } => {
            if free < 1 {
                return Err(());
            }

            push(spot_view(settings, center, direction_dst, range, fov), f32::MAX);
        }
        StandardLight::Directional { direction_dst, .. } => {
            let count = settings.cascade_count.clamp(1, 4) as usize;

            if free < count {
                return Err(());
            }

            let near = camera.camera.near;
            let far = camera.camera.far.min(settings.distance.max(near));
            let mut split_near = near;

            for split_far in cascade_splits(near, far, count, settings.split_lambda) {
                push(
                    cascade_view(settings, camera, direction_dst, split_near, split_far),
                    split_far,
                );
                split_near = split_far;
            }
        }
    }

    Ok((start as u32, (views.len() as usize - start) as u32))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn approx(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-3
    }

    #[test]
    fn splits_end_at_far_and_increase() {
        let splits = cascade_splits(0.1, 50.0, 4, 0.75).collect::<Vec<_>>();

        assert_eq!(splits.len(), 4);
        assert!(approx(splits[3], 50.0));
        assert!(splits.windows(2).all(|pair| pair[0] < pair[1]));

        let uniform = cascade_splits(0.0, 40.0, 4, 0.0).collect::<Vec<_>>();
        assert!(approx(uniform[0], 0.001 + (40.0 - 0.001) * 0.25));
    }

    #[test]
    fn tiles_form_a_grid() {
        let settings = ShadowSettingsResource {
            atlas_size: 4096,
            tile_size: 1024,
            ..Default::default()
        };

        assert_eq!(tiles_count(&settings), 16);
        assert_eq!(tile_rect(&settings, 0), Vec4::new(0.0, 0.0, 0.25, 0.25));
        assert_eq!(tile_rect(&settings, 5), Vec4::new(0.25, 0.25, 0.25, 0.25));
        assert_eq!(tile_rect(&settings, 15), Vec4::new(0.75, 0.75, 0.25, 0.25));
    }

    #[test]
    fn look_rotation_is_proper() {
        for forward in [
            Vec3::new(0.0, -1.0, 0.0),
            Vec3::new(0.3, -0.8, 0.5).normalized(),
            Vec3::new(0.0, 0.0, 1.0),
        ] {
            let rotation = look_rotation(forward);

            assert!(approx(rotation.determinant(), 1.0));
            assert_eq!(rotation.as_array()[2], forward.into_array());
        }
    }

    fn camera() -> ShadowCamera {
        let camera = Camera::default();
        let rotation = Mat3::from_array([[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]]);
        let view = rotation.into_4x4_with_offset(Vec3::splat(0.0)).inverse().unwrap();

        ShadowCamera {
            world_to_clip: camera.projection_matrix(1.0) * view,
            camera,
        }
    }

    #[test]
    fn cascade_contains_its_frustum_slice() {
        let settings = ShadowSettingsResource::default();
        let world_to_clip = cascade_view(&settings, &camera(), Vec3::new(0.0, -1.0, 0.0), 0.1, 5.0);

        for point in [Vec3::new(0.0, 0.0, 1.0), Vec3::new(4.0, 4.0, 4.9), Vec3::new(-4.0, -4.0, 4.9)] {
            let ndc = world_to_clip.mul_with_projection(point);

            assert!(ndc.x.abs() <= 1.0 && ndc.y.abs() <= 1.0, "{ndc:?}");
            assert!((0.0..=1.0).contains(&ndc.z), "{ndc:?}");
        }
    }

    #[test]
    fn lights_take_tiles_until_full() {
        let settings = ShadowSettingsResource {
            atlas_size: 2048,
            tile_size: 1024,
            cascade_count: 3,
            ..Default::default()
        };
        let camera = camera();
        let mut views = FfiVec::new();

        let directional = StandardLight::Directional {
            color: Vec3::splat(1.0),
            direction_dst: Vec3::new(0.0, -1.0, 0.0),
            shadows: true,
        };
        let spot = StandardLight::Spot {
            color: Vec3::splat(1.0),
            center: Vec3::splat(0.0),
            range: 10.0,
            direction_dst: Vec3::new(0.0, -1.0, 0.0),
            fov: 1.0,
            shadows: true,
        };
        let point = StandardLight::Point {
            color: Vec3::splat(1.0),
            center: Vec3::splat(0.0),
            range: 10.0,
            shadows: true,
        };

        assert_eq!(push_light_shadow_views(&settings, &camera, &point, &mut views), Ok((0, 0)));
        assert_eq!(
            push_light_shadow_views(&settings, &camera, &directional, &mut views),
            Ok((0, 3))
        );
        assert_eq!(push_light_shadow_views(&settings, &camera, &spot, &mut views), Ok((3, 1)));
        assert_eq!(push_light_shadow_views(&settings, &camera, &spot, &mut views), Err(()));
        assert_eq!(views.len(), 4);
        assert_eq!(views[3].atlas_rect, Vec4::new(0.5, 0.5, 0.5, 0.5));
    }
}
