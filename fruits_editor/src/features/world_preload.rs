use std::{ffi::OsStr, path::{Path, PathBuf}, sync::{Arc, Mutex}, time::Duration};

use fruits_engine::*;
use libloading::Library;

use crate::{PROJECT_ASSETS_SUBPATH, SYSTEM_GROUP, app_building, features::inspector_window::data::OpenProjectResource};

pub fn register_feature(mut world: WorldBuilderMut) {
    world.data_mut().resources_mut().insert(SimulatedWorldResource(None));

    let mut behavior = world.behavior_mut();
    let mut update = behavior.get_mut(Schedule::Update);

    update.group(SYSTEM_GROUP)
        .insert_child_system(preload_assets_into_simulated_world_system)
        .insert_child_system(tick_simulated_world_system);

    update.order_system(preload_assets_into_simulated_world_system)
        .before_system(tick_simulated_world_system);
}

pub struct SimulatedWorld {
    pub world: World,
    pub library: Option<Library>,
    pub is_started: bool,
}
impl SimulatedWorld {
    pub fn new(world: World, library: Option<Library>) -> Self {
        Self {
            world,
            library,
            is_started: false,
        }
    }
}

#[derive(Resource)]
pub struct SimulatedWorldResource(pub Option<SimulatedWorld>);

fn preload_assets_into_simulated_world_system(
    open_project: Res<OpenProjectResource>,
    mut simulated_world: ResMut<SimulatedWorldResource>,
    render_api: Res<RenderApiResource>,
) {
    if simulated_world.0.is_some() {
        return;
    }

    let lib_path = app_building::path_scripts_lib_src(&open_project.dir_path);
    let lib_path = copy_lib_to_unique(&open_project.dir_path, &lib_path);

    let mut lib = None;

    let mut world = WorldBuilder::new();

    world.data_mut().resources_mut().insert({
        let mut serializers = SerializersResource::default();
        register_common_transserializers(&mut *serializers);
        serializers
    });

    if let Some(lib_path) = lib_path {
        let init_result = unsafe {
            init_app_dynamically(world.as_mut(), lib_path)
        };
        match init_result {
            Ok(loaded_lib) => lib = Some(loaded_lib),
            Err(err) => {
                println!("failed to load scripts lib: {err}");
            },
        }
    } else {
        println!("failed to load scripts lib");
    }

    world.behavior_mut().clear();

    add_collision_module_to(world.as_mut());
    add_transform_module_to(world.as_mut());
    add_ui_module_to(world.as_mut());
    add_render_module_to(world.as_mut());

    world
        .behavior_mut()
        .get_mut(Schedule::Update)
        .order_group(SYSTEM_GROUP_COLLISION)
        .before_group(SYSTEM_GROUP_TRANSFORM)
        .before_group(SYSTEM_GROUP_RENDER);

    let mut world = world.build();

    let mut res = world.data_mut().into_resources_mut();

    let serializer = res.get_mut::<SerializersResource>().unwrap();
    let serializer = &mut serializer.0;

    serializer.register(StandardSerializer::<SerializedValue>::default());
    serializer.register(StandardSerializer::<RenderSpace>::default());
    serializer.register(StandardSerializer::<f32>::default());
    serializer.register(StandardSerializer::<bool>::default());
    serializer.register(StandardSerializer::<FfiString>::default());
    serializer.register(StandardSerializer::<FfiSmallString>::default());
    serializer.register(StandardSerializer::<CoordinateSpaceType>::default());
    serializer.register(StandardSerializer::<Vec4<f32>>::default());
    serializer.register(StandardSerializer::<FfiOption<f32>>::default());
    serializer.register(StandardSerializer::<Option<f32>>::default());
    serializer.register(StandardSerializer::<StandardTextureAssetMetadata>::default());
    serializer.register(StandardSerializer::<StandardMaterialAssetMetadata>::default());
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
    res.insert(AudioStateResource::new(FfiDroppable::new(()), WrappedAudioStateHandle::new(Arc::new(Mutex::new(AudioState::new())))));
    res.insert(AssetStorageResource::<AudioClip>::new());
    res.insert(AssetStorageResource::<Prefab>::new());

    load_all_assets(res.as_mut(), &(open_project.dir_path.to_string() + PROJECT_ASSETS_SUBPATH));

    simulated_world.0 = Some(SimulatedWorld::new(world, lib));
}

// todo: make simulated world ticking panic-safe and freeze-safe.
fn tick_simulated_world_system(mut simulated_world: ResMut<SimulatedWorldResource>) {
    let Some(simulated_world) = &mut simulated_world.0 else {
        return;
    };

    if !simulated_world.is_started {
        simulated_world.world.execute_iteration(Schedule::Start);
        simulated_world.is_started = true;
    }

    simulated_world.world.execute_iteration(Schedule::Update);

    simulated_world.world.data_mut().events_mut().clear();
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

    let timestamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or(Duration::ZERO).as_millis();
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