use std::{
    ffi::OsStr,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use aphyn::*;
use libloading::Library;

use crate::{
    PROJECT_ASSETS_SUBPATH, SYSTEM_GROUP,
    app_building::{self, FileStamp},
    features::{
        inspector_window::data::{EntityPath, InspectedEntityResource, OpenProjectResource},
        project_reload::ProjectReloadResource,
        transform_gizmo::{TRANSFORM_GIZMO_RENDER_FEATURE, TransformGizmoRenderFeature},
    },
};

pub fn register_feature(ctx: FeatureInitCtx) {
    let FeatureInitCtx {
        mut data, mut behavior, ..
    } = ctx;
    data.resources_mut().insert(SimulatedWorldResource(None, None));

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
    pub library_stamp: Option<FileStamp>,
    pub mode: SimulationMode,
    pub threading: ThreadingMode,
    pub is_started: bool,
    pub last_tick: Option<Instant>,
    pub profiler: Option<ProfileRecorder>,
}
impl SimulatedWorld {
    pub fn new(
        data: WorldData,
        behavior: WorldBehavior,
        render: RenderFeatureRegistry,
        (library, library_stamp): (Option<Library>, Option<FileStamp>),
        mode: SimulationMode,
        threading: ThreadingMode,
    ) -> Self {
        Self {
            data,
            behavior,
            render,
            library,
            library_stamp,
            mode,
            threading,
            is_started: false,
            last_tick: None,
            profiler: None,
        }
    }

    pub fn start_if_needed(&mut self, thread_pool: ThreadPoolAccess) {
        if !self.is_started {
            self.behavior
                .execute_iteration(Schedule::Start, self.data.as_mut(), thread_pool, self.profiler.as_ref());
            self.is_started = true;
        }
    }
}

// The pool is created on first use and kept across world rebuilds.
#[derive(Resource)]
pub struct SimulatedWorldResource(pub Option<SimulatedWorld>, Option<ThreadPool>);

impl SimulatedWorldResource {
    pub fn is_playing(&self) -> bool {
        self.0.as_ref().is_some_and(|w| w.mode == SimulationMode::Play)
    }

    pub fn frame_threads_count(&self) -> usize {
        match self.0.as_ref().map(|w| w.threading) {
            Some(ThreadingMode::ThreadPool) => self.1.as_ref().map_or(0, ThreadPool::threads_count),
            _ => 0,
        }
    }

    pub fn world_and_thread_pool(&mut self) -> Option<(&mut SimulatedWorld, ThreadPoolAccess<'_>)> {
        let world = self.0.as_mut()?;

        let thread_pool = match world.threading {
            ThreadingMode::ThreadPool => ThreadPoolAccess::from(&*self.1.get_or_insert_with(ThreadPool::default)),
            ThreadingMode::SingleThreaded => ThreadPoolAccess::single_threaded(),
        };

        Some((world, thread_pool))
    }
}

#[derive(Resource)]
struct PlaySceneResource {
    asset_key: FfiString,
}

pub fn preload_assets_into_simulated_world_system(
    open_project: Res<OpenProjectResource>,
    project_reload: Res<ProjectReloadResource>,
    mut simulated_world: ResMut<SimulatedWorldResource>,
    render_api: Res<RenderApiResource>,
) {
    if simulated_world.0.is_some() || !project_reload.is_initial_build_done() {
        return;
    }

    let lib = load_scripts_library(&open_project.dir_path);
    let (data, behavior, render, threading) =
        build_simulated_world(&open_project.dir_path, &render_api, lib.0.as_ref(), SimulationMode::Edit, "");

    simulated_world.0 = Some(SimulatedWorld::new(
        data,
        behavior,
        render,
        lib,
        SimulationMode::Edit,
        threading,
    ));
}

// the scene is usually spawned right after a rebuild; a few frames cover systems that spawn it on Start
const RESELECT_FRAMES: u32 = 30;

pub fn rebuild_simulated_world(
    simulated_world: &mut SimulatedWorldResource,
    inspected_entity: &mut InspectedEntityResource,
    project_dir: &str,
    render_api: &RenderApiResource,
    mode: SimulationMode,
    play_scene_key: &str,
    reload_library: bool,
) {
    let reselect = simulated_world
        .0
        .as_ref()
        .and_then(|w| EntityPath::of(w.data.as_ref().entities(), inspected_entity.selected_entity))
        .map(|path| (path, RESELECT_FRAMES));

    // the old world holds the library's types and function pointers, so it must be dropped before the library is reused
    let library = simulated_world.0.take().map(
        |SimulatedWorld {
             data,
             behavior,
             render,
             library,
             library_stamp,
             ..
         }| {
            drop(behavior);
            drop(data);
            drop(render);
            (library, library_stamp)
        },
    );

    // shown components may hold values allocated by the library
    *inspected_entity = InspectedEntityResource {
        pending_reselect: reselect,
        ..Default::default()
    };

    let library = match library {
        Some(library) if !reload_library => library,
        library => {
            drop(library);
            load_scripts_library(project_dir)
        }
    };

    let (data, behavior, render, threading) =
        build_simulated_world(project_dir, render_api, library.0.as_ref(), mode, play_scene_key);

    simulated_world.0 = Some(SimulatedWorld::new(data, behavior, render, library, mode, threading));
}

fn load_scripts_library(project_dir: &str) -> (Option<Library>, Option<FileStamp>) {
    let lib_path = app_building::path_scripts_lib_src(project_dir);
    let lib_stamp = app_building::file_stamp(&lib_path);

    let Some(lib_path) = copy_lib_to_unique(project_dir, &lib_path) else {
        println!("failed to load scripts lib");
        return (None, lib_stamp);
    };

    match unsafe { Library::new(lib_path) } {
        Ok(lib) => (Some(lib), lib_stamp),
        Err(err) => {
            println!("failed to load scripts lib: {err}");
            (None, lib_stamp)
        }
    }
}

fn build_simulated_world(
    project_dir: &str,
    render_api: &RenderApiResource,
    lib: Option<&Library>,
    mode: SimulationMode,
    play_scene_key: &str,
) -> (WorldData, WorldBehavior, RenderFeatureRegistry, ThreadingMode) {
    let assets_dir = project_dir.to_string() + PROJECT_ASSETS_SUBPATH;

    let mut data = WorldData::default();
    let types = data.types().clone();
    let mut behavior = WorldBehaviorBuilder::new(data.types().clone());
    let mut render = RenderFeatureRegistry::default();
    let mut serializers = TransSerializerRegistryBuilder::new();
    let mut reflection = ReflectionRegistryBuilder::new();

    match mode {
        SimulationMode::Edit => {
            register_common_transserializers(&types, &mut serializers);
            // the inspector edits through it; the scripts lib extends it with its own types
            register_common_reflectors(&types, &mut reflection);
        }
        SimulationMode::Play => add_default_modules_to(FeatureInitCtx {
            data: data.as_mut(),
            types: &types,
            behavior: behavior.as_mut(),
            render: &mut render,
            serializers: &mut serializers,
            reflection: &mut reflection,
        }),
    }

    let mut window_state = WindowState {
        size: [1, 1],
        title: "Scene".into(),
        ..Default::default()
    };

    let mut threading = ThreadingMode::default();

    if let Some(lib) = lib
        && let Err(err) = unsafe {
            init_app_from_library(
                AppInitCtx {
                    data: data.as_mut(),
                    types: &types,
                    behavior: behavior.as_mut(),
                    window: &mut window_state,
                    render: &mut render,
                    threading: &mut threading,
                    serializers: &mut serializers,
                    reflection: &mut reflection,
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
            data.resources_mut().insert(TransformSettingsResource {
                recalculate_static: true,
            });
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

    render.add_feature(TRANSFORM_GIZMO_RENDER_FEATURE, TransformGizmoRenderFeature::default());

    let behavior = behavior.build();

    {
        let serializer = &mut serializers;

        serializer.register(
            &types,
            "aphyn::SerializedValue",
            StandardSerializer::<SerializedValue>::default(),
        );
        serializer.register(&types, "aphyn::RenderSpace", StandardSerializer::<RenderSpace>::default());
        serializer.register(
            &types,
            "aphyn::StandardTextureFormat",
            StandardSerializer::<StandardTextureFormat>::default(),
        );
        serializer.register(&types, "f32", StandardSerializer::<f32>::default());
        serializer.register(&types, "bool", StandardSerializer::<bool>::default());
        serializer.register(&types, "aphyn::FfiString", StandardSerializer::<FfiString>::default());
        serializer.register(
            &types,
            "aphyn::FfiSmallString",
            StandardSerializer::<FfiSmallString>::default(),
        );
        serializer.register(
            &types,
            "aphyn::CoordinateSpaceType",
            StandardSerializer::<CoordinateSpaceType>::default(),
        );
        serializer.register(&types, "aphyn::Vec4<f32>", StandardSerializer::<Vec4<f32>>::default());
        serializer.register(
            &types,
            "aphyn::FfiOption<f32>",
            StandardSerializer::<FfiOption<f32>>::default(),
        );
        serializer.register(&types, "Option<f32>", StandardSerializer::<Option<f32>>::default());
        serializer.register(
            &types,
            "aphyn::StandardTextureAssetMetadata",
            StandardSerializer::<StandardTextureAssetMetadata>::default(),
        );
        serializer.register(
            &types,
            "aphyn::ShaderAssetMetadata",
            StandardSerializer::<ShaderAssetMetadata>::default(),
        );
        serializer.register(
            &types,
            "aphyn::ComputeShaderAssetMetadata",
            StandardSerializer::<ComputeShaderAssetMetadata>::default(),
        );
        serializer.register(
            &types,
            "aphyn::StandardMeshAssetMetadata",
            StandardSerializer::<StandardMeshAssetMetadata>::default(),
        );
        serializer.register(
            &types,
            "aphyn::AudioClipAssetMetadata",
            StandardSerializer::<AudioClipAssetMetadata>::default(),
        );
        serializer.register(
            &types,
            "aphyn::AnimationClipAssetMetadata",
            StandardSerializer::<AnimationClipAssetMetadata>::default(),
        );
        // todo
        // serializer.register(StandardTransSerializer::<Prefab>::default());
        serializer.register(
            &types,
            "aphyn::FfiOption<aphyn::AssetHandle<aphyn::StandardTexture>>",
            StandardSerializer::<FfiOption<AssetHandle<StandardTexture>>>::default(),
        );
        serializer.register(&types, "aphyn::NameComponent", StandardSerializer::<NameComponent>::default());
        serializer.register(
            &types,
            "aphyn::ChildComponent",
            StandardSerializer::<ChildComponent>::default(),
        );
        serializer.register(
            &types,
            "aphyn::ParentComponent",
            StandardSerializer::<ParentComponent>::default(),
        );
        serializer.register(
            &types,
            "aphyn::StaticComponent",
            StandardSerializer::<StaticComponent>::default(),
        );
        serializer.register(
            &types,
            "aphyn::FfiVec<aphyn::EntityId>",
            StandardSerializer::<FfiVec<EntityId>>::default(),
        );
    }

    {
        let mut res = data.resources_mut();

        res.insert(SerializersResource(serializers.build()));
        res.insert(ReflectionRegistryResource(reflection.build()));
        res.insert(render_api.clone());
        res.insert(WindowResource::new(window_state));
        res.insert(TimeResource::default());

        match mode {
            SimulationMode::Edit => {
                res.insert(AudioStateResource::new(
                    FfiDroppable::new(()),
                    WrappedAudioStateHandle::new(Arc::new(Mutex::new(AudioState::new()))),
                ));
                res.insert(AssetStorageResource::<AudioClip>::new());
                res.insert(AssetStorageResource::<AnimationClip>::new());
                res.insert(AssetStorageResource::<Prefab>::new());
                res.insert(AssetStorageResource::<Scene>::new());

                // asset files may reference the built-in templates, which Start would register too late
                insert_builtin_render_assets_to(res.as_mut());
                load_all_assets(res.as_mut(), &assets_dir);
            }
            SimulationMode::Play => _ = res.insert(InputResource::default()),
        }
    }

    (data, behavior, render, threading)
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
    let Some((simulated_world, thread_pool)) = simulated_world.world_and_thread_pool() else {
        return;
    };

    simulated_world.start_if_needed(thread_pool);

    let now = Instant::now();
    let real_delta_time = simulated_world
        .last_tick
        .map_or(0.0, |last| now.duration_since(last).as_secs_f32());
    simulated_world.last_tick = Some(now);

    run_frame_schedules(
        &mut simulated_world.behavior,
        simulated_world.data.as_mut(),
        real_delta_time,
        thread_pool,
        simulated_world.profiler.as_ref(),
    );

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
