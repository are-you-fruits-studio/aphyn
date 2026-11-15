use aphyn_ecs::{ThreadPoolAccess, WorldDataMut};
use aphyn_ffi::FfiVec;
use aphyn_math::Vec3;
use aphyn_render::{
    CurrentRenderingResource, FilterMode, LIGHTS_COUNT_MAX, RenderApiResource, StandardGenericLight, StandardLight,
    StandardTextureAssetMetadata, StandardTextureFormat, StandardUniformGlobal,
};
use aphyn_transform::GlobalTransform;

use crate::{
    AmbientLightResource, DepthTextureResource, GlobalBindingsResource, RENDER_ORDER_PREPARE, RenderFeature, RenderPassWriter,
    ScreenSpaceResource, ShadowAtlasResource, ShadowSettingsResource, StandardLightComponent, TransparentTargetResource,
    create_window_to_clip_matrix, light_from_components,
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
        for light in self.lights.iter() {
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
        let lights = &self.generic_lights;

        if is_atlas_full && !self.logged_atlas_full {
            self.logged_atlas_full = true;
            eprintln!("some lights get no shadows: the shadow atlas has no free tiles left");
        }

        atlas.prepare(render_api.core(), shadow_settings.atlas_size);
        let has_shadows = !atlas.views().is_empty();

        let globals = &res.get::<GlobalBindingsResource>().unwrap().bindings;
        let atlas = res.get::<ShadowAtlasResource>().unwrap();

        render_api.write_lights_with(globals, lights.len() as u32, |i| lights[i as u64]);
        render_api.write_shadow_views(globals, atlas.views());
        render_api.write_global_uniform(
            globals,
            StandardUniformGlobal {
                world_to_clip,
                window_to_clip: create_window_to_clip_matrix(target_size[0] as f32, target_size[1] as f32, near, far),
                camera_position_world: camera_position,
                lights_count: lights.len() as u32,
                ambient_color,
                shadow_bias: shadow_settings.bias,
                camera_forward_world: camera_forward,
                shadow_normal_offset: shadow_settings.normal_offset,
            },
        );

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
                core.clear_depth_texture(depth.texture.texture(), 0, 1.0);
            }

            if let Some(transparent) = resources.get::<TransparentTargetResource>() {
                core.clear_color_texture(transparent.texture.texture(), 0, [0.0; 4]);
            }

            if has_shadows && let Some(atlas) = resources.get::<ShadowAtlasResource>() {
                core.clear_depth_texture(atlas.texture(), 0, 1.0);
            }
        });
    }
}
