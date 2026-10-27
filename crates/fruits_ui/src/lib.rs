use fruits_asset_storage::AssetStorageResource;
use fruits_ecs::{Schedule, WorldBuilderMut};
use fruits_render::SYSTEM_GROUP_RENDER;
use fruits_transform::SYSTEM_GROUP_TRANSFORM;

mod assets;
mod components;
mod resources;
mod systems;

pub use {assets::*, components::*, resources::*, systems::*};

pub const SYSTEM_GROUP_UI: &'static str = "fruits_ui";

pub fn add_ui_module_to(mut world: WorldBuilderMut) {
    let mut res = world.data_mut().into_resources_mut();
    
    res.insert(AssetStorageResource::<Font>::new());

    let mut world_behavior = world.behavior_mut();

    let mut start = world_behavior.get_mut(Schedule::Start);

    start
        .group(SYSTEM_GROUP_UI)
        .insert_child_system(create_standard_ui_assets_resource);

    let mut update = world_behavior.get_mut(Schedule::Update);

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
