// #![windows_subsystem = "windows"]

mod components;
mod features;
mod prefabs;
mod resources;
mod systems;
mod theme;
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
// - advanced hierarchy window interaction (move entity)
// + copy/paste entity
// + copy/paste component
//
// --- ABILITY TO MAKE NICE EDITOR UI ---
//
// - create GUI for selecting INSPECTED_ASSETS_PATH (for opening a project)

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

    let mut ctx = app.init_ctx_mut();

    *ctx.threading = ThreadingMode::SingleThreaded;

    add_default_modules_to(ctx.feature_ctx());

    {
        let mut start = ctx.behavior.get_mut(Schedule::Start);

        start.insert_system(init_system);

        start
            .order_system(create_standard_ui_assets_resource)
            .before_system(init_system);
        start.order_group(SYSTEM_GROUP_RENDER).before_system(init_system);
    }

    {
        let mut update = ctx.behavior.get_mut(Schedule::Update);

        update.order_group(SYSTEM_GROUP).before_group(SYSTEM_GROUP_RENDER);
        update.order_group(SYSTEM_GROUP).before_group(SYSTEM_GROUP_TRANSFORM);
    }

    features::world_preload::register_feature(ctx.feature_ctx());
    features::clipboard::register_feature(ctx.feature_ctx());
    features::project_reload::register_feature(ctx.feature_ctx());
    features::scroll::register_feature(ctx.feature_ctx());
    features::ui_interaction::register_feature(ctx.feature_ctx());
    features::project_window_entries::register_feature(ctx.feature_ctx());
    features::project_window_selection::register_feature(ctx.feature_ctx());
    features::project_window_rename::register_feature(ctx.feature_ctx());
    features::project_window_create::register_feature(ctx.feature_ctx());
    features::inspector_window::register_feature(ctx.feature_ctx());
    features::input_field::register_feature(ctx.feature_ctx());
    features::number_drag::register_feature(ctx.feature_ctx());
    features::dropdown::register_feature(ctx.feature_ctx());
    features::generic_dropdown::register_feature(ctx.feature_ctx());
    features::serialization::register_feature(ctx.feature_ctx());
    features::test_window::register_feature(ctx.feature_ctx());
    features::ui_window::register_feature(ctx.feature_ctx());
    features::docking::register_feature(ctx.feature_ctx());
    features::windows_layout::register_feature(ctx.feature_ctx());
    features::scene_window::register_feature(ctx.feature_ctx());
    features::scene_light::register_feature(ctx.feature_ctx());
    features::transform_gizmo::register_feature(ctx.feature_ctx());
    features::game_window::register_feature(ctx.feature_ctx());
    features::play_mode::register_feature(ctx.feature_ctx());
    features::animation_window::register_feature(ctx.feature_ctx());
    features::profiler_window::register_feature(ctx.feature_ctx());
    features::foldout::register_feature(ctx.feature_ctx());

    ctx.data.resources_mut().insert(OpenProjectResource {
        dir_path: project_path.to_string(),
    });

    {
        let mut ec = ctx.data.entities_mut();

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

    ctx.window.fullscreen = FullscreenState::WindowedMaximized;

    app.run();
}

fn init_system(mut world: WorldDataMut) {
    let (mut res, _ec, _evt) = world.as_tuple_mut();

    let mut load_editor_font = |data: &[u8], size: f32, weight: f32| {
        let mut settings = FontRasterSettings::new(size);
        settings.weight = Some(weight);

        let render_api = unsafe { &*res.get_ptr::<RenderApiResource>().unwrap() };
        let textures = res.get_mut::<AssetStorageResource<StandardTexture>>().unwrap();
        let font = load_font(render_api, textures, data, &settings).unwrap();

        let mut text_data = MaterialData::new();
        text_data.insert("color_tex", MaterialDataValue::Texture(font.texture.clone()));
        let material = create_material_from_template_key(res.as_ref(), BUILTIN_TEMPLATE_UI_BLENDED_UNLIT, text_data).unwrap();

        let font = res.get_mut::<AssetStorageResource<Font>>().unwrap().insert(font);
        let material = res.get_mut::<AssetStorageResource<Material>>().unwrap().insert(material);
        (font, material)
    };

    let (font, material_text) = load_editor_font(include_bytes!("../assets/fonts/Onest.ttf"), theme::FONT, 400.0);
    let (font_mono, material_text_mono) =
        load_editor_font(include_bytes!("../assets/fonts/JetBrainsMono.ttf"), theme::FONT_MONO, 400.0);

    let material_panel =
        create_material_from_template_key(res.as_ref(), BUILTIN_TEMPLATE_UI_OPAQUE_UNLIT, MaterialData::new()).unwrap();
    let material_panel = res
        .get_mut::<AssetStorageResource<Material>>()
        .unwrap()
        .insert(material_panel);

    res.insert(UiInteractionResource::default());

    res.insert(StandardAssetsResource {
        material_panel,
        material_text,
        font,
        material_text_mono,
        font_mono,
    });

    prefabs::editor_window(world.as_mut());
    // prefabs::project_window(world.as_mut());
    // prefabs::hierarhy_window(world.as_mut());
    // prefabs::inspector_window(world.as_mut());
}
//
