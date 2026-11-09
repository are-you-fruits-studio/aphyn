use std::{
    ffi::OsStr,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::Duration,
};

use aphyn::*;
use libloading::Library;

use crate::{PROJECT_ASSETS_SUBPATH, SYSTEM_GROUP, app_building, features::inspector_window::data::OpenProjectResource};

// todo: bundle registration params
pub fn register_feature(mut data: WorldDataMut, mut behavior: WorldBehaviorBuilderMut, _render: &mut RenderFeatureRegistry) {
    data.resources_mut().insert(SimulatedWorldResource(None));

    let mut update = behavior.get_mut(Schedule::Update);

    update
        .group(SYSTEM_GROUP)
        .insert_child_system(preload_assets_into_simulated_world_system)
        .insert_child_system(tick_simulated_world_system);

    update
        .order_system(preload_assets_into_simulated_world_system)
        .before_system(tick_simulated_world_system);
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum SimulationMode {
    Edit,
    Play,
}

pub struct SimulatedWorld {
    pub data: WorldData,
    pub behavior: WorldBehavior,
    pub render: RenderFeatureRegistry,
    pub library: Option<Library>,
    pub mode: SimulationMode,
    pub is_started: bool,
}
impl SimulatedWorld {
    pub fn new(
        data: WorldData,
        behavior: WorldBehavior,
        render: RenderFeatureRegistry,
        library: Option<Library>,
        mode: SimulationMode,
    ) -> Self {
        Self {
            data,
            behavior,
            render,
            library,
            mode,
            is_started: false,
        }
    }

    pub fn start_if_needed(&mut self) {
        if !self.is_started {
            self.behavior.execute_iteration(Schedule::Start, self.data.as_mut());
            self.is_started = true;
        }
    }
}

#[derive(Resource)]
pub struct SimulatedWorldResource(pub Option<SimulatedWorld>);

impl SimulatedWorldResource {
    pub fn is_playing(&self) -> bool {
        self.0.as_ref().is_some_and(|w| w.mode == SimulationMode::Play)
    }
}

#[derive(Resource)]
struct PlaySceneResource {
    asset_key: FfiString,
}

pub fn preload_assets_into_simulated_world_system(
    open_project: Res<OpenProjectResource>,
    mut simulated_world: ResMut<SimulatedWorldResource>,
    render_api: Res<RenderApiResource>,
) {
    if simulated_world.0.is_some() {
        return;
    }

    let lib = load_scripts_library(&open_project.dir_path);
    let (data, behavior, render) =
        build_simulated_world(&open_project.dir_path, &render_api, lib.as_ref(), SimulationMode::Edit, "");

    simulated_world.0 = Some(SimulatedWorld::new(data, behavior, render, lib, SimulationMode::Edit));
}

pub fn rebuild_simulated_world(
    simulated_world: &mut SimulatedWorldResource,
    project_dir: &str,
    render_api: &RenderApiResource,
    mode: SimulationMode,
    play_scene_key: &str,
) {
    // the old world holds the library's types and function pointers, so it must be dropped before the library is reused
    let library = simulated_world.0.take().and_then(
        |SimulatedWorld {
             data,
             behavior,
             render,
             library,
             ..
         }| {
            drop(behavior);
            drop(data);
            drop(render);
            library
        },
    );

    let (data, behavior, render) = build_simulated_world(project_dir, render_api, library.as_ref(), mode, play_scene_key);

    simulated_world.0 = Some(SimulatedWorld::new(data, behavior, render, library, mode));
}

fn load_scripts_library(project_dir: &str) -> Option<Library> {
    let lib_path = app_building::path_scripts_lib_src(project_dir);

    let Some(lib_path) = copy_lib_to_unique(project_dir, &lib_path) else {
        println!("failed to load scripts lib");
        return None;
    };

    match unsafe { Library::new(lib_path) } {
        Ok(lib) => Some(lib),
        Err(err) => {
            println!("failed to load scripts lib: {err}");
            None
        }
    }
}

fn build_simulated_world(
    project_dir: &str,
    render_api: &RenderApiResource,
    lib: Option<&Library>,
    mode: SimulationMode,
    play_scene_key: &str,
) -> (WorldData, WorldBehavior, RenderFeatureRegistry) {
    let assets_dir = project_dir.to_string() + PROJECT_ASSETS_SUBPATH;

    let mut data = WorldData::default();
    let mut behavior = WorldBehaviorBuilder::new(data.types().clone());
    let mut render = RenderFeatureRegistry::default();

    match mode {
        SimulationMode::Edit => {
            _ = data.resources_mut().insert({
                let mut serializers = SerializersResource::default();
                register_common_transserializers(&mut *serializers);
                serializers
            })
        }
        SimulationMode::Play => add_defult_modules_to(data.as_mut(), behavior.as_mut(), &mut render),
    }

    let mut window_state = WindowState {
        size: [1, 1],
        title: "Scene".into(),
        ..Default::default()
    };

    if let Some(lib) = lib
        && let Err(err) = unsafe {
            init_app_from_library(
                AppInitCtx {
                    data: data.as_mut(),
                    behavior: behavior.as_mut(),
                    window: &mut window_state,
                    render: &mut render,
                },
                lib,
            )
        }
    {
        println!("failed to init scripts lib: {err}");
    }

    match mode {
        SimulationMode::Edit => {
            behavior.as_mut().clear();

            add_collision_module_to(data.as_mut(), behavior.as_mut(), &mut render);
            add_transform_module_to(data.as_mut(), behavior.as_mut());
            add_ui_module_to(data.as_mut(), behavior.as_mut(), &mut render);
            add_render_features_module_to(data.as_mut(), behavior.as_mut(), &mut render);

            behavior
                .as_mut()
                .get_mut(Schedule::Update)
                .order_group(SYSTEM_GROUP_COLLISION)
                .before_group(SYSTEM_GROUP_TRANSFORM)
                .before_group(SYSTEM_GROUP_RENDER);
        }
        SimulationMode::Play => {
            data.resources_mut().insert(AssetsDirResource(assets_dir.as_str().into()));
            data.resources_mut().insert(PlaySceneResource {
                asset_key: play_scene_key.into(),
            });

            let mut behavior = behavior.as_mut();
            let mut start = behavior.get_mut(Schedule::Start);

            start.group(SYSTEM_GROUP_ASSETS).insert_child_system(spawn_play_scene_system);
            start
                .order_system(load_all_assets_system)
                .before_system(spawn_play_scene_system);
        }
    }

    let behavior = behavior.build();

    {
        let mut res = data.resources_mut();

        let serializer = res.get_mut::<SerializersResource>().unwrap();
        let serializer = &mut serializer.0;

        serializer.register(StandardSerializer::<SerializedValue>::default());
        serializer.register(StandardSerializer::<RenderSpace>::default());
        serializer.register(StandardSerializer::<StandardTextureFormat>::default());
        serializer.register(StandardSerializer::<f32>::default());
        serializer.register(StandardSerializer::<bool>::default());
        serializer.register(StandardSerializer::<FfiString>::default());
        serializer.register(StandardSerializer::<FfiSmallString>::default());
        serializer.register(StandardSerializer::<CoordinateSpaceType>::default());
        serializer.register(StandardSerializer::<Vec4<f32>>::default());
        serializer.register(StandardSerializer::<FfiOption<f32>>::default());
        serializer.register(StandardSerializer::<Option<f32>>::default());
        serializer.register(StandardSerializer::<StandardTextureAssetMetadata>::default());
        serializer.register(StandardSerializer::<ShaderAssetMetadata>::default());
        serializer.register(StandardSerializer::<StandardMeshAssetMetadata>::default());
        serializer.register(StandardSerializer::<AudioClipAssetMetadata>::default());
        // todo
        // serializer.register(StandardTransSerializer::<Prefab>::default());
        serializer.register(StandardSerializer::<FfiOption<AssetHandle<StandardTexture>>>::default());
        serializer.register(StandardSerializer::<DebugNameComponent>::default());
        serializer.register(StandardSerializer::<ChildComponent>::default());
        serializer.register(StandardSerializer::<ParentComponent>::default());
        serializer.register(StandardSerializer::<FfiVec<EntityId>>::default());

        res.insert(render_api.clone());
        res.insert(WindowResource::new(window_state));

        match mode {
            SimulationMode::Edit => {
                res.insert(AudioStateResource::new(
                    FfiDroppable::new(()),
                    WrappedAudioStateHandle::new(Arc::new(Mutex::new(AudioState::new()))),
                ));
                res.insert(AssetStorageResource::<AudioClip>::new());
                res.insert(AssetStorageResource::<Prefab>::new());
                res.insert(AssetStorageResource::<Scene>::new());

                load_all_assets(res.as_mut(), &assets_dir);
            }
            SimulationMode::Play => _ = res.insert(InputResource::default()),
        }
    }

    (data, behavior, render)
}

fn spawn_play_scene_system(mut world: WorldDataMut) {
    let (res, ent, _) = world.as_tuple_mut();

    return_if_not!(Some(play_scene) = res.get::<PlaySceneResource>());
    return_if_not!(
        Some(scene) = res
            .get::<AssetStorageResource<Scene>>()
            .and_then(|s| s.get_registered(play_scene.asset_key.as_str()))
            .cloned()
    );

    instantiate_scene(res.as_ref(), ent, scene);
}

// todo: make simulated world ticking panic-safe and freeze-safe.
pub fn tick_simulated_world_system(mut simulated_world: ResMut<SimulatedWorldResource>) {
    let Some(simulated_world) = &mut simulated_world.0 else {
        return;
    };

    simulated_world.start_if_needed();

    simulated_world
        .behavior
        .execute_iteration(Schedule::Update, simulated_world.data.as_mut());

    simulated_world.data.events_mut().clear();
}

fn copy_lib_to_unique(project_path: impl AsRef<Path>, lib_path: impl AsRef<Path>) -> Option<PathBuf> {
    // todo: handle std::fs errs.

    let lib_path = lib_path.as_ref();

    let Ok(true) = std::fs::exists(lib_path) else {
        return None;
    };

    let mut project_path = project_path.as_ref().to_owned();
    project_path.push("tmp");
    let tmp_dir_path = project_path;
    _ = std::fs::remove_dir_all(&tmp_dir_path);
    _ = std::fs::create_dir_all(&tmp_dir_path);

    let lib_ext = lib_path.extension();
    let lib_stem = lib_path.file_stem();

    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or(Duration::ZERO)
        .as_millis();
    let mut new_file_name = lib_stem.unwrap_or(OsStr::new("")).to_owned();
    new_file_name.push(OsStr::new(&timestamp.to_string()));
    if let Some(lib_ext) = lib_ext {
        new_file_name.push(OsStr::new("."));
        new_file_name.push(lib_ext);
    }
    let mut new_file_path = tmp_dir_path;
    new_file_path.push(new_file_name);

    _ = std::fs::copy(lib_path, &new_file_path);

    Some(new_file_path)
}
