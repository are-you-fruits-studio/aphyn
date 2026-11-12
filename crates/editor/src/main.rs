// #![windows_subsystem = "windows"]

mod components;
mod features;
mod prefabs;
mod resources;
mod systems;
mod utils;

use crate::{
    components::*,
    features::{inspector_window::data::OpenProjectResource, ui_interaction::*},
    resources::*,
    utils::*,
};

use aphyn::*;

mod app_building;

const SYSTEM_GROUP: &'static str = "aphyn_editor";

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
        if let Err(err) = app_building::build_app(&args[2]) {
            eprintln!("build failed: {err}");
            std::process::exit(1);
        }
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

    let AppInitCtx {
        mut data,
        mut behavior,
        window,
        render,
    } = app.init_ctx_mut();

    add_defult_modules_to(data.as_mut(), behavior.as_mut(), render);

    {
        let mut start = behavior.get_mut(Schedule::Start);

        start.insert_system(init_system);

        start
            .order_system(create_standard_ui_assets_resource)
            .before_system(init_system);
        start.order_group(SYSTEM_GROUP_RENDER).before_system(init_system);
    }

    {
        let mut update = behavior.get_mut(Schedule::Update);

        update.order_group(SYSTEM_GROUP).before_group(SYSTEM_GROUP_RENDER);
        update.order_group(SYSTEM_GROUP).before_group(SYSTEM_GROUP_TRANSFORM);
    }

    features::world_preload::register_feature(data.as_mut(), behavior.as_mut(), render);
    features::scroll::register_feature(data.as_mut(), behavior.as_mut(), render);
    features::ui_interaction::register_feature(data.as_mut(), behavior.as_mut(), render);
    features::project_window_entries::register_feature(data.as_mut(), behavior.as_mut(), render);
    features::project_window_selection::register_feature(data.as_mut(), behavior.as_mut(), render);
    features::project_window_rename::register_feature(data.as_mut(), behavior.as_mut(), render);
    features::project_window_create::register_feature(data.as_mut(), behavior.as_mut(), render);
    features::inspector_window::register_feature(data.as_mut(), behavior.as_mut(), render);
    features::input_field::register_feature(data.as_mut(), behavior.as_mut(), render);
    features::number_drag::register_feature(data.as_mut(), behavior.as_mut(), render);
    features::dropdown::register_feature(data.as_mut(), behavior.as_mut(), render);
    features::generic_dropdown::register_feature(data.as_mut(), behavior.as_mut(), render);
    features::serialization::register_feature(data.as_mut(), behavior.as_mut(), render);
    features::test_window::register_feature(data.as_mut(), behavior.as_mut(), render);
    features::ui_window::register_feature(data.as_mut(), behavior.as_mut(), render);
    features::windows_layout::register_feature(data.as_mut(), behavior.as_mut(), render);
    features::scene_window::register_feature(data.as_mut(), behavior.as_mut(), render);
    features::game_window::register_feature(data.as_mut(), behavior.as_mut(), render);
    features::play_mode::register_feature(data.as_mut(), behavior.as_mut(), render);
    features::foldout::register_feature(data.as_mut(), behavior.as_mut(), render);

    data.resources_mut().insert(OpenProjectResource {
        dir_path: project_path.to_string(),
    });

    {
        let mut ec = data.entities_mut();

        let camera = ec.create_entity();
        ec.add_component(camera, GlobalTransform::default()).ok().unwrap();
        ec.add_component(
            camera,
            CameraComponent(Camera {
                near: 0.1,
                far: 1_000.0,
                projection: CameraProjection::Perspective {
                    fov: 90_f32.to_radians(),
                },
            }),
        )
        .ok()
        .unwrap();
    }

    window.fullscreen = FullscreenState::WindowedMaximized;

    app.run();
}

fn init_system(mut world: WorldDataMut) {
    let (mut res, _ec, _evt) = world.as_tuple_mut();

    let standard_ui_assets = res.get::<StandardUiAssetsResource>().unwrap();

    let font = standard_ui_assets.font_px_8_12.clone();
    let texture_text = standard_ui_assets.texture_text_px_8_8.clone();

    let material_panel =
        create_material_from_template_key(res.as_ref(), BUILTIN_TEMPLATE_UI_OPAQUE_UNLIT, MaterialData::new()).unwrap();

    let mut text_data = MaterialData::new();
    text_data.insert("color_tex", MaterialDataValue::Texture(texture_text.clone()));
    let material_text = create_material_from_template_key(res.as_ref(), BUILTIN_TEMPLATE_UI_OPAQUE_UNLIT, text_data).unwrap();

    let materials_res = res.get_mut::<AssetStorageResource<Material>>().unwrap();

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
