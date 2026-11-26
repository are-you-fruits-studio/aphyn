use aphyn_ecs::{ThreadPoolAccess, WorldDataMut};
use aphyn_ffi::FfiVec;
use aphyn_math::{Mat4, Vec3, Vec4};
use aphyn_render::{
    CLUSTER_DIMS, CurrentRenderingResource, FilterMode, LIGHTS_COUNT_MAX, RenderApiResource, StandardGenericLight, StandardLight,
    StandardTextureAssetMetadata, StandardTextureFormat, StandardUniformGlobal,
};
use aphyn_time::TimeResource;
use aphyn_transform::GlobalTransform;

use crate::{
    AmbientLightResource, ClusteredLightingResource, DepthTextureResource, GlobalBindingsResource, RENDER_ORDER_PREPARE,
    RenderFeature, RenderPassWriter, SceneNormalsTextureResource, ScreenSpaceResource, ShadowAtlasResource, ShadowSettingsResource,
    StandardLightComponent, TransparentTargetResource, create_window_to_clip_matrix, light_from_components,
    shadows::{ShadowCamera, push_light_shadow_views},
};

#[repr(C)]
#[derive(Default)]
pub struct GlobalsRenderFeature {
    lights: FfiVec<StandardLight>,
    generic_lights: FfiVec<StandardGenericLight>,
    logged_atlas_full: bool,
}

impl RenderFeature for GlobalsRenderFeature {
    fn add_passes(&mut self, world: WorldDataMut, _thread_pool: ThreadPoolAccess, passes: &mut RenderPassWriter<Self>) {
        let (mut res, entities, _) = world.into_tuple_mut();

        let Some(render_api) = res.get::<RenderApiResource>().map(RenderApiResource::clone) else {
            return;
        };
        let Some(current) = res.get::<CurrentRenderingResource>() else {
            return;
        };

        let target_size = current.target_size();
        let world_to_clip = current.world_to_clip();
        let world_to_view = current.world_to_view();
        let view_to_clip = current.view_to_clip();
        let camera = current.camera;
        let clustering = res.get::<ClusteredLightingResource>().copied().unwrap_or_default();
        let camera_position = current.camera_position;
        let camera_forward = (current.camera_scale_rotation * Vec3::new(0.0, 0.0, 1.0)).normalized();
        let shadow_camera = ShadowCamera {
            world_to_clip,
            camera: current.camera,
        };
        let view_light = current.light;
        let ambient_color = current
            .ambient_light
            .or_else(|| res.get::<AmbientLightResource>().map(|ambient| ambient.color))
            .unwrap_or(Vec3::splat(0.0));
        let (near, far) = res
            .get::<ScreenSpaceResource>()
            .map_or((-10_000.0, 10_000.0), |screen_space| (screen_space.near, screen_space.far));
        let shadow_settings = res.get::<ShadowSettingsResource>().copied().unwrap_or_default();
        let time = res.get::<TimeResource>().cloned().unwrap_or_default();
        let time_phases = [8.0, 4.0, 2.0, 1.0].map(|divisor| time.frame_time / divisor);

        if res.get::<GlobalBindingsResource>().is_none() {
            res.insert(GlobalBindingsResource {
                bindings: render_api.create_global_bindings(),
            });
        }

        if res.get::<ShadowAtlasResource>().is_none() {
            res.insert(ShadowAtlasResource::new(render_api.core()));
        }

        // todo: reusable allocation-free ECS query
        let lights_query = entities.query::<(Option<&GlobalTransform>, &StandardLightComponent)>();
        self.lights.clear();
        for light in lights_query
            .iter()
            .map(|(transform, light)| light_from_components(light, &transform.copied().unwrap_or_default()))
            .chain(view_light)
            .take(LIGHTS_COUNT_MAX)
        {
            self.lights.push(light);
        }

        let atlas = res.get_mut::<ShadowAtlasResource>().unwrap();
        let shadow_views = atlas.views_mut();
        shadow_views.clear();

        let mut is_atlas_full = false;
        self.generic_lights.clear();
        // directional lights first: clustering only assigns the local lights after them
        let is_directional = |light: &StandardLight| matches!(light, StandardLight::Directional { .. });
        let directional = self.lights.iter().filter(|light| is_directional(light));
        let local = self.lights.iter().filter(|light| !is_directional(light));
        for light in directional.chain(local) {
            let (shadow_start, shadow_count) = push_light_shadow_views(&shadow_settings, &shadow_camera, light, shadow_views)
                .unwrap_or_else(|()| {
                    is_atlas_full = true;
                    (0, 0)
                });

            self.generic_lights.push(StandardGenericLight {
                shadow_start,
                shadow_count,
                ..(*light).into()
            });
        }
        let directional_lights_count = self.lights.iter().filter(|light| is_directional(light)).count() as u32;
        let lights = &self.generic_lights;

        if is_atlas_full && !self.logged_atlas_full {
            self.logged_atlas_full = true;
            eprintln!("some lights get no shadows: the shadow atlas has no free tiles left");
        }

        atlas.prepare(render_api.core(), shadow_settings.atlas_size);
        let has_shadows = !atlas.views().is_empty();

        let globals = &mut res.get_mut::<GlobalBindingsResource>().unwrap().bindings;
        render_api.set_cluster_pairs_capacity(globals, clustering.max_light_cluster_pairs as usize);

        let globals = &res.get::<GlobalBindingsResource>().unwrap().bindings;
        let atlas = res.get::<ShadowAtlasResource>().unwrap();

        let cluster_near = camera.near.max(0.001);
        let uniform = StandardUniformGlobal {
            world_to_clip,
            window_to_clip: create_window_to_clip_matrix(target_size[0] as f32, target_size[1] as f32, near, far),
            world_to_view,
            clip_to_view: view_to_clip.inverse().unwrap_or(Mat4::IDENTITY),
            camera_position_world: camera_position,
            lights_count: lights.len() as u32,
            ambient_color,
            shadow_bias: shadow_settings.bias,
            camera_forward_world: camera_forward,
            shadow_normal_offset: shadow_settings.normal_offset,
            cluster_dims: CLUSTER_DIMS,
            directional_lights_count,
            target_size: target_size.map(|x| x as f32),
            cluster_near,
            cluster_far: camera.far.max(cluster_near * 1.01),
            cluster_index_capacity: globals.cluster_pairs_capacity() as u32,
            // todo: f32 time loses precision in long sessions, making shader animation stutter after hours
            time: time.frame_time as f32,
            delta_time: time.delta_time,
            frame_number: time.frame_number as u32,
            time_sin: Vec4::from_array(time_phases.map(|phase| phase.sin() as f32)),
            time_cos: Vec4::from_array(time_phases.map(|phase| phase.cos() as f32)),
        };

        render_api.write_lights_with(globals, lights.len() as u32, |i| lights[i as u64]);
        render_api.write_shadow_views(globals, atlas.views());
        render_api.write_global_uniform(globals, uniform);

        let create_target = |format| {
            render_api.create_texture(
                FilterMode::Nearest,
                target_size,
                &[],
                StandardTextureAssetMetadata {
                    format,
                    ..Default::default()
                },
            )
        };

        if res
            .get::<DepthTextureResource>()
            .is_none_or(|depth| depth.texture.size() != target_size)
        {
            res.insert(DepthTextureResource {
                texture: create_target(StandardTextureFormat::Depth32Float),
            });
        }

        if res
            .get::<SceneNormalsTextureResource>()
            .is_none_or(|normals| normals.texture.size() != target_size)
        {
            res.insert(SceneNormalsTextureResource {
                texture: create_target(StandardTextureFormat::Rg16Float),
            });
        }

        if res
            .get::<TransparentTargetResource>()
            .is_none_or(|transparent| transparent.texture.size() != target_size)
        {
            res.insert(TransparentTargetResource {
                texture: create_target(StandardTextureFormat::Rgba16Float),
            });
        }

        passes.add(RENDER_ORDER_PREPARE, move |_, ctx| {
            let core = ctx.render_api.core();
            let resources = ctx.world.resources();

            if let Some(depth) = resources.get::<DepthTextureResource>() {
                core.clear_depth_texture(Some("clear depth"), depth.texture.texture(), 0, 0.0);
            }

            if let Some(normals) = resources.get::<SceneNormalsTextureResource>() {
                core.clear_color_texture(Some("clear scene normals"), normals.texture.texture(), 0, [0.0; 4]);
            }

            if let Some(transparent) = resources.get::<TransparentTargetResource>() {
                core.clear_color_texture(Some("clear transparent target"), transparent.texture.texture(), 0, [0.0; 4]);
            }

            if has_shadows && let Some(atlas) = resources.get::<ShadowAtlasResource>() {
                core.clear_depth_texture(Some("clear shadow atlas"), atlas.texture(), 0, 0.0);
            }
        });
    }
}
