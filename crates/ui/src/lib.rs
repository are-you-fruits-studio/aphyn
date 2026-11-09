use aphyn_asset_storage::AssetStorageResource;
use aphyn_ecs::{Schedule, WorldBehaviorBuilderMut, WorldDataMut};
use aphyn_render_features::SYSTEM_GROUP_RENDER;
use aphyn_render_features::RenderFeatureRegistry;
use aphyn_transform::SYSTEM_GROUP_TRANSFORM;

mod assets;
mod components;
mod resources;
mod systems;

pub use {assets::*, components::*, resources::*, systems::*};

pub const SYSTEM_GROUP_UI: &'static str = "aphyn_ui";

// todo: bundle registration params
pub fn add_ui_module_to(mut data: WorldDataMut, mut behavior: WorldBehaviorBuilderMut, _render: &mut RenderFeatureRegistry) {
    let mut res = data.as_mut().into_resources_mut();

    res.insert(AssetStorageResource::<Font>::new());

    let mut start = behavior.get_mut(Schedule::Start);

    start
        .group(SYSTEM_GROUP_UI)
        .insert_child_system(create_standard_ui_assets_resource);

    let mut update = behavior.get_mut(Schedule::Update);

    update
        .group(SYSTEM_GROUP_UI)
        .insert_child_system(update_text_batched_mesh)
        .insert_child_system(update_image_batched_mesh)
        .insert_child_system(update_masked_batched_mesh);

    update
        .order_system(update_text_batched_mesh)
        .before_system(update_masked_batched_mesh);
    update
        .order_system(update_image_batched_mesh)
        .before_system(update_masked_batched_mesh);

    update
        .order_group(SYSTEM_GROUP_TRANSFORM)
        .before_group(SYSTEM_GROUP_UI)
        .before_group(SYSTEM_GROUP_RENDER);
}
