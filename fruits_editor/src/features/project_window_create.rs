use std::path::{Path, PathBuf};

use crate::{
    features::{
        asset_serialization::get_asset_type,
        generic_dropdown::{GenericDropdownResource, select_generic_dropdown_system},
        inspector_window::{
            systems::{
                change_inspected_asset_system, save_inspected_asset_from_simulated_world_to_file_system,
                save_simulated_entities_to_asset_system,
            },
            utils::serialization::{enrich_serialized_with_asset_type, load_asset_to_world_res, save_asset_from_world_res},
        },
        project_window_rename::{ProjectEntryRenameResource, get_non_clashing_path, start_project_entry_rename_system},
        project_window_selection::{
            FileSelectedEvent, SelectedFileResource, apply_project_entry_context_menu_system, get_asset_key,
            get_potential_asset_key, update_project_entry_selection_system,
        },
        world_preload::SimulatedWorldResource,
    },
    *,
};

pub fn register_feature(mut world: WorldBuilderMut) {
    world
        .data_mut()
        .resources_mut()
        .insert(ProjectEntryCreateMenuResource::default());

    let mut behavior = world.behavior_mut();
    let mut update = behavior.get_mut(Schedule::Update);

    update
        .group(SYSTEM_GROUP)
        .insert_child_system(apply_project_entry_create_menu_system);

    update
        .order_system(select_generic_dropdown_system)
        .before_system(apply_project_entry_create_menu_system);

    update
        .order_system(apply_project_entry_context_menu_system)
        .before_system(apply_project_entry_create_menu_system);

    update
        .order_system(apply_project_entry_create_menu_system)
        .before_system(save_simulated_entities_to_asset_system);

    update
        .order_system(apply_project_entry_create_menu_system)
        .before_system(save_inspected_asset_from_simulated_world_to_file_system);

    update
        .order_system(apply_project_entry_create_menu_system)
        .before_system(change_inspected_asset_system);

    update
        .order_system(apply_project_entry_create_menu_system)
        .before_system(update_project_entry_selection_system);

    update
        .order_system(apply_project_entry_create_menu_system)
        .before_system(start_project_entry_rename_system);
}

#[derive(Clone, Copy)]
enum ProjectEntryCreateKind {
    Folder,
    Material,
    Prefab,
    Scene,
    Texture,
    Mesh,
    AudioClip,
}

impl ProjectEntryCreateKind {
    const ALL: [Self; 7] = [
        Self::Folder,
        Self::Material,
        Self::Prefab,
        Self::Scene,
        Self::Texture,
        Self::Mesh,
        Self::AudioClip,
    ];

    fn label(self) -> &'static str {
        match self {
            Self::Folder => "Folder",
            Self::Material => "Material",
            Self::Prefab => "Prefab",
            Self::Scene => "Scene",
            Self::Texture => "Texture",
            Self::Mesh => "Mesh",
            Self::AudioClip => "Audio clip",
        }
    }

    fn default_stem(self) -> &'static str {
        match self {
            Self::Folder => "New Folder",
            Self::Material => "New Material",
            Self::Prefab => "New Prefab",
            Self::Scene => "New Scene",
            Self::Texture => "New Texture",
            Self::Mesh => "New Mesh",
            Self::AudioClip => "New Audio Clip",
        }
    }

    fn asset_type(self) -> Option<AssetType> {
        match self {
            Self::Folder => None,
            Self::Material => Some(AssetType::Material),
            Self::Prefab => Some(AssetType::Prefab),
            Self::Scene => Some(AssetType::Scene),
            Self::Texture => Some(AssetType::Texture),
            Self::Mesh => Some(AssetType::Mesh),
            Self::AudioClip => Some(AssetType::AudioClip),
        }
    }
}

#[derive(Resource, Default)]
pub struct ProjectEntryCreateMenuResource {
    request_id: u64,
    dir: PathBuf,
}

impl ProjectEntryCreateMenuResource {
    pub fn request(&mut self, generic_dropdown: &mut GenericDropdownResource, dir: PathBuf, position: Vec2<f32>) {
        let labels = ProjectEntryCreateKind::ALL
            .iter()
            .map(|k| FfiString::from(k.label()))
            .collect();
        self.request_id = generic_dropdown.request(labels, position);
        self.dir = dir;
    }
}

pub fn apply_project_entry_create_menu_system(
    generic_dropdown_res: Res<GenericDropdownResource>,
    open_project: Res<OpenProjectResource>,
    mut create_menu_res: ResMut<ProjectEntryCreateMenuResource>,
    mut simulated_world: ResMut<SimulatedWorldResource>,
    mut selected_file: ResMut<SelectedFileResource>,
    mut select_evt: EvtMut<FileSelectedEvent>,
    mut rename_res: ResMut<ProjectEntryRenameResource>,
) {
    return_if_not!(Some(idx) = generic_dropdown_res.take_selected(&mut create_menu_res.request_id));

    let kind = ProjectEntryCreateKind::ALL[idx as usize];

    let path = match kind.asset_type() {
        None => create_folder(&create_menu_res.dir, kind.default_stem()),
        Some(asset_type) => create_asset(
            &create_menu_res.dir,
            kind.default_stem(),
            asset_type,
            &open_project,
            &mut simulated_world,
        ),
    };
    return_if_not!(Some(path) = path);

    let path = path.into_os_string();

    selected_file.potential_asset_key = get_potential_asset_key(&path, &open_project);
    selected_file.path = path.clone();
    select_evt.push(FileSelectedEvent);

    rename_res.request(path);
}

fn create_folder(dir: &Path, stem: &str) -> Option<PathBuf> {
    let path = get_non_clashing_path(dir, stem, None);

    if let Err(err) = std::fs::create_dir(&path) {
        eprintln!("failed to create folder {:?}. {}", path, err);
        return None;
    }

    Some(path)
}

fn create_asset(
    dir: &Path,
    stem: &str,
    asset_type: AssetType,
    open_project: &OpenProjectResource,
    simulated_world: &mut SimulatedWorldResource,
) -> Option<PathBuf> {
    let path = get_non_clashing_path(dir, stem, Some("asset"));

    let Some(key) = get_asset_key(path.as_os_str(), open_project) else {
        eprintln!("failed to create asset {:?}. path is outside of the assets directory", path);
        return None;
    };

    let Some(simulated_world) = &mut simulated_world.0 else {
        eprintln!("failed to create asset {:?}. simulated world is not loaded", path);
        return None;
    };

    let mut res = simulated_world.world.data_mut().into_resources_mut();

    let serialized = match asset_type {
        AssetType::Texture => serialize_default_metadata::<StandardTextureAssetMetadata>(res.as_ref(), asset_type),
        AssetType::Material => serialize_default_metadata::<StandardMaterialAssetMetadata>(res.as_ref(), asset_type),
        AssetType::Mesh => serialize_default_metadata::<StandardMeshAssetMetadata>(res.as_ref(), asset_type),
        AssetType::AudioClip => serialize_default_metadata::<AudioClipAssetMetadata>(res.as_ref(), asset_type),
        AssetType::Prefab => {
            let name = path.file_stem().and_then(|s| s.to_str()).unwrap_or(stem);
            create_named_root_prefab(res.as_ref(), name).and_then(|prefab| {
                register_asset(res.as_mut(), &key, prefab);
                save_asset_from_world_res(res.as_ref(), &key)
            })
        }
        AssetType::Scene => {
            register_asset(res.as_mut(), &key, Scene::empty());
            save_asset_from_world_res(res.as_ref(), &key)
        }
        AssetType::Font => None,
    };

    let Some(serialized) = serialized else {
        eprintln!("failed to create asset {:?}. serialization failed", path);
        return None;
    };

    let json_str = serde_json::to_string_pretty(&serialized.to_json()).unwrap();

    if let Err(err) = std::fs::write(&path, json_str.into_bytes()) {
        eprintln!("failed to create asset {:?}. {}", path, err);
        return None;
    }

    if matches!(
        asset_type,
        AssetType::Texture | AssetType::Material | AssetType::Mesh | AssetType::AudioClip
    ) {
        let assets_dir_path = open_project.dir_path.to_string() + PROJECT_ASSETS_SUBPATH;
        load_asset_to_world_res(res.as_mut(), &key, &serialized, asset_type, &assets_dir_path);
    }

    if get_asset_type(res.as_ref(), &key) != Some(asset_type) {
        eprintln!("failed to load created asset {:?}", path);
    }

    Some(path)
}

fn serialize_default_metadata<T: 'static + Default>(res: ResourcesHolderRef, asset_type: AssetType) -> Option<SerializedValue> {
    let mut serialized = serialize_with_world_serializers(res, &T::default())?;
    enrich_serialized_with_asset_type(&mut serialized, asset_type);
    Some(serialized)
}

fn create_named_root_prefab(res: ResourcesHolderRef, name: &str) -> Option<Prefab> {
    let component_name = FfiString::from(FfiExtendedTypeInfo::of::<DebugNameComponent>().short().name());
    let component = serialize_with_world_serializers(res, &DebugNameComponent(name.into()))?;

    let components = SerializedEntityComponents([(component_name, component)].into_iter().collect());

    Some(Prefab {
        entities: SerializedEntities([(1, components)].into_iter().collect()),
    })
}

fn serialize_with_world_serializers<T: 'static>(res: ResourcesHolderRef, value: &T) -> Option<SerializedValue> {
    let serializers_global = &**res.get::<SerializersResource>()?;

    let mut err_handler = |err| eprintln!("[{}:{}] {err}", file!(), line!());

    save_load_with_asset_serializers_from_world(res, |serializers_local| {
        let serializer_ctx_state = serializers_global.to_ctx_state();
        let mut serializer_ctx = serializer_ctx_state
            .wrap_with_local(&serializers_local)
            .into_ctx(&mut err_handler);

        serializer_ctx.serialize(value, "")
    })
}

fn register_asset<T: 'static + Send + Sync>(res: ResourcesHolderMut, key: &str, asset: T) {
    return_if_not!(Some(storage) = res.into_get_mut::<AssetStorageResource<T>>());
    storage.insert_and_register(asset, key.into());
}
