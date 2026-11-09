use aphyn_ecs::WorldDataMut;
use aphyn_render::{
    CurrentRenderingResource, FilterMode, LIGHTS_COUNT_MAX, RenderApiResource, StandardTextureAssetMetadata, StandardTextureFormat,
    StandardUniformGlobal,
};
use aphyn_transform::GlobalTransform;

use crate::{
    DepthTextureResource, GlobalBindingsResource, RENDER_ORDER_PREPARE, RenderFeature, RenderPassWriter, ScreenSpaceResource,
    StandardLightComponent, TransparentTargetResource, create_window_to_clip_matrix, light_from_components,
};

#[derive(Default)]
pub struct GlobalsRenderFeature;

impl RenderFeature for GlobalsRenderFeature {
    fn add_passes(&mut self, world: WorldDataMut, passes: &mut RenderPassWriter<Self>) {
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
        let view_light = current.light;
        let (near, far) = res
            .get::<ScreenSpaceResource>()
            .map_or((-10_000.0, 10_000.0), |screen_space| (screen_space.near, screen_space.far));

        if res.get::<GlobalBindingsResource>().is_none() {
            res.insert(GlobalBindingsResource {
                bindings: render_api.create_global_bindings(),
            });
        }

        let globals = &res.get::<GlobalBindingsResource>().unwrap().bindings;

        // todo: reusable allocation-free ECS query
        let lights_query = entities.query::<(Option<&GlobalTransform>, &StandardLightComponent)>();
        let mut lights = lights_query
            .iter()
            .map(|(transform, light)| light_from_components(light, &transform.copied().unwrap_or_default()))
            .chain(view_light);
        let lights_count = (lights_query.len() as usize + view_light.is_some() as usize).min(LIGHTS_COUNT_MAX) as u32;

        // elements are written in index order, so the iterator can be consumed sequentially
        render_api.write_lights_with(globals, lights_count, |_| lights.next().map(Into::into).unwrap_or_default());
        render_api.write_global_uniform(
            globals,
            StandardUniformGlobal {
                world_to_clip,
                window_to_clip: create_window_to_clip_matrix(target_size[0] as f32, target_size[1] as f32, near, far),
                camera_position_world: camera_position,
                lights_count,
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

        passes.add(RENDER_ORDER_PREPARE, |_, ctx| {
            let core = ctx.render_api.core();
            let resources = ctx.world.resources();

            if let Some(depth) = resources.get::<DepthTextureResource>() {
                core.clear_depth_texture(depth.texture.texture(), 0, 1.0);
            }

            if let Some(transparent) = resources.get::<TransparentTargetResource>() {
                core.clear_color_texture(transparent.texture.texture(), 0, [0.0; 4]);
            }
        });
    }
}
