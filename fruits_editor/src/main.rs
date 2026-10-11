// #![windows_subsystem = "windows"]

mod components;
mod features;
mod prefabs;
mod resources;
mod systems;
mod utils;

use crate::{
    components::*, features::{inspector_window::data::OpenProjectResource, ui_interaction::*}, resources::*, utils::*,
};

use fruits_engine::*;

mod app_building;

const SYSTEM_GROUP: &'static str = "fruits_editor";

// todo:
// + show entity names or ids if name component is missing
// + show entities in hierarchy
// + highlight selected entity
// + show selected entity components
// + show entities parent-child-relation in hierarchy
// + inspect prefab's entity's components (load/save component details)
// + basic hierarchy window interaction (create/delete entity)
// + basic inspector window interaction (add/remove component)
// + save prefabs (apply inspector changes to world, then to asset, then to file)
// + use dynamic INSPECTED_ASSETS_PATH - from command line args
// + load custom serializers on start
// + use custom components in prefabs
//
// --- DONE ---
// --- START EDITING EDITOR ASSETS ---
//
// - window manager
// - preview (scene) window
// - editor-time simulated systems
//
// --- ABILITY TO MAKE ADEQUATE EDITOR UI ---
//
// - advanced hierarchy window interaction (move/copy/paste entity)
// - advanced inspector window interaction (copy/paste component)
//
// --- ABILITY TO MAKE NICE EDITOR UI ---
//
// - create GUI for selecting INSPECTED_ASSETS_PATH (for opening a project)
// - recompile project automatically or at least with a button.

// todo: to dynamic (user-selected)
const PROJECT_ASSETS_SUBPATH: &'static str = "/assets/";

fn main() {
    let args = std::env::args()
        .collect::<Vec<_>>()
        .join(" ")
        .split_whitespace()
        .map(|s| s.to_string())
        .collect::<Vec<_>>();

    if args.len() < 3 {
        return;
    }

    if args[1].as_str() == "build" {
        app_building::build_app(&args[2]);
    }

    if args[1].as_str() == "run" {
        todo!();
    }

    if args[1].as_str() == "edit" {
        run_editor_app(&args[2]);
    }
}

fn run_editor_app(project_path: &str) {
    let mut app = App::new();

    let world = app.ecs_mut();

    add_defult_modules_to(world.as_mut());

    let mut world_behavior = world.behavior_mut();

    {
        let mut start = world_behavior.get_mut(Schedule::Start);

        start.insert_system(init_system);
        start.insert_system(maximize_window_system);

        start.order_system(create_standard_ui_assets_resource)
            .before_system(init_system);
    }

    {
        let mut update = world_behavior.get_mut(Schedule::Update);

        update.order_group(SYSTEM_GROUP).before_group(SYSTEM_GROUP_RENDER);
        update.order_group(SYSTEM_GROUP).before_group(SYSTEM_GROUP_TRANSFORM);
    }

    features::world_preload::register_feature(world.as_mut());
    features::scroll::register_feature(world.as_mut());
    features::ui_interaction::register_feature(world.as_mut());
    features::project_window_entries::register_feature(world.as_mut());
    features::project_window_selection::register_feature(world.as_mut());
    features::inspector_window::register_feature(world.as_mut());
    features::input_field::register_feature(world.as_mut());
    features::dropdown::register_feature(world.as_mut());
    features::generic_dropdown::register_feature(world.as_mut());
    features::serialization::register_feature(world.as_mut());
    features::test_window::register_feature(world.as_mut());
    features::ui_window::register_feature(world.as_mut());
    features::windows_layout::register_feature(world.as_mut());

    world.as_mut().data_mut().resources_mut().insert(OpenProjectResource { dir_path: project_path.to_string() });

    app.run();
}

fn maximize_window_system(mut window: ResMut<WindowResource>) {
    window.next_state_mut().fullscreen = FullscreenState::WindowedMaximized;
}

fn init_system(mut world: WorldDataMut) {
    let (mut res, ec, evt) = world.as_tuple_mut();

    let standard_ui_assets = res.get::<StandardUiAssetsResource>().unwrap();

    let font = standard_ui_assets.font_px_8_12.clone();
    let texture_text = standard_ui_assets.texture_text_px_8_8.clone();

    let textures_res = res.get::<AssetStorageResource<StandardTexture>>().unwrap();

    let material_panel = StandardMaterialAssetMetadata {
        is_lit: false,
        space: RenderSpace::Window,
        color: Vec4::splat(1.0),
        color_tex: AssetHandle::EMPTY,
        alpha_threshold: Some(0.5).into(),
        ..Default::default()
    };

    let material_text = StandardMaterialAssetMetadata {
        is_lit: false,
        space: RenderSpace::Window,
        color: Vec4::splat(1.0),
        color_tex: texture_text.clone(),
        alpha_threshold: Some(0.5).into(),
        ..Default::default()
    };

    let texture_text_asset = textures_res.get(&texture_text);

    let material_panel = res.as_ref()
        .get::<RenderApiResource>()
        .unwrap()
        .create_material(Default::default(), material_panel);

    let material_text = res.as_ref()
        .get::<RenderApiResource>()
        .unwrap()
        .create_material(StandardMaterialAssets {
            color_texture: texture_text_asset,
            ..Default::default()
        }, material_text);

    let materials_res = res.get_mut::<AssetStorageResource<StandardMaterial>>().unwrap();

    let material_panel = materials_res.insert(material_panel);
    let material_text = materials_res.insert(material_text);

    res.insert(UiInteractionResource::default());

    res.insert(StandardAssetsResource {
        material_panel: material_panel.clone(),
        material_text: material_text.clone(),
        font,
    });

    prefabs::editor_window(world.as_mut());
    // prefabs::project_window(world.as_mut());
    // prefabs::hierarhy_window(world.as_mut());
    // prefabs::inspector_window(world.as_mut());
}
//
