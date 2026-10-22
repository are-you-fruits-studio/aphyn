use std::{
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
};

use notify::{EventKind, RecursiveMode, Watcher as _, event::ModifyKind};

use crate::{
    features::{
        foldout::{FoldoutKey, FoldoutResource, spawn_foldout_arrow}, inspector_window::utils::entries::spawn_hierarchy_entry, project_window_selection::get_potential_asset_key,
    },
    prefabs::WindowComponent,
    *,
};

pub fn register_feature(mut world: WorldBuilderMut) {
    world
        .data_mut()
        .resources_mut()
        .insert(ProjectWindowCache::default());

    let mut behavior = world.behavior_mut();
    let mut update = behavior.get_mut(Schedule::Update);

    update.group(SYSTEM_GROUP)
        .insert_child_system(update_project_window_content_system);

    update
        .order_system(update_project_window_content_system)
        .before_system(prepare_ui_raycast_system);
}

#[derive(Resource, Default)]
pub struct ProjectWindowCache {
    pub dir_entry: ProjectWindowDataEntry,
    watcher: Option<ProjectDirWatcher>,
}

struct ProjectDirWatcher {
    path: PathBuf,
    is_dirty: Arc<AtomicBool>,
    watcher: Option<Mutex<notify::RecommendedWatcher>>,
}

impl ProjectDirWatcher {
    fn new(path: PathBuf) -> Self {
        let is_dirty = Arc::new(AtomicBool::new(true));
        let is_dirty_handler = is_dirty.clone();

        let watcher = notify::recommended_watcher(move |event: notify::Result<notify::Event>| {
            let affects_entries = match event {
                Ok(event) => !matches!(
                    event.kind,
                    EventKind::Access(_) | EventKind::Modify(ModifyKind::Data(_) | ModifyKind::Metadata(_))
                ),
                Err(_) => true,
            };

            if affects_entries {
                is_dirty_handler.store(true, Ordering::Relaxed);
            }
        })
        .and_then(|mut watcher| watcher.watch(&path, RecursiveMode::Recursive).map(|_| watcher));

        let watcher = match watcher {
            Ok(watcher) => Some(Mutex::new(watcher)),
            Err(err) => {
                eprintln!("failed to watch {:?}, rescanning every frame. {}", path, err);
                None
            }
        };

        Self { path, is_dirty, watcher }
    }

    fn take_dirty(&self) -> bool {
        self.watcher.is_none() || self.is_dirty.swap(false, Ordering::Relaxed)
    }
}

pub fn update_project_window_content_system(mut world: WorldDataMut) {
    let (mut res, mut ent, evt) = world.as_tuple_mut();

    let assets = res.as_ref().get::<StandardAssetsResource>().unwrap().clone();
    let open_project = res.as_ref().get::<OpenProjectResource>().unwrap().clone();

    let font = assets.font.clone();

    let windows = ent
        .query_filtered::<EntityId, WithFilter<ProjectWindowComponent>>()
        .iter()
        .collect::<Vec<_>>();

    let current_dir = PathBuf::from(open_project.dir_path.clone() + PROJECT_ASSETS_SUBPATH);

    let foldout_res = std::mem::take(res.get_mut::<FoldoutResource>().unwrap());

    let cache = res.get_mut::<ProjectWindowCache>().unwrap();

    let is_dirty = match &cache.watcher {
        Some(watcher) if watcher.path == current_dir => watcher.take_dirty(),
        _ => {
            cache.watcher = Some(ProjectDirWatcher::new(current_dir.clone()));
            true
        }
    };

    let is_cache_old = is_dirty && {
        let entry = ProjectWindowDataEntry::scan(&current_dir);
        let is_changed = cache.dir_entry != entry;

        if is_changed {
            cache.dir_entry = entry;
        }

        is_changed
    };

    let entry = &cache.dir_entry;

    for ent_window in windows {
        let project_win_c = ent.get_component_mut::<ProjectWindowComponent>(ent_window).unwrap();

        if !is_cache_old && project_win_c.is_init {
            continue;
        }

        project_win_c.is_init = true;

        let content = ent.get_component::<WindowComponent>(ent_window).unwrap().content_container;

        destroy_entity_children(ent.as_mut(), content);

        for entry in &entry.children {
            spawn_project_window_entries(
                ent.as_mut(),
                &assets.material_text,
                &assets.material_panel,
                &font,
                &open_project,
                &foldout_res,
                content,
                entry,
            );
        }
    }

    *res.get_mut::<FoldoutResource>().unwrap() = foldout_res;
}

fn spawn_project_window_entries(
    mut ec: EntitiesHolderMut,
    material_text: &AssetHandle<StandardMaterial>,
    material_panel: &AssetHandle<StandardMaterial>,
    font: &AssetHandle<Font>,
    open_project: &OpenProjectResource,
    foldout_res: &FoldoutResource,
    parent: EntityId,
    entry: &ProjectWindowDataEntry,
) {
    let spawned = spawn_hierarchy_entry(
        ec.as_mut(),
        material_text,
        material_panel,
        font,
        parent,
        entry.name.clone().into(),
    );

    ec.set_components(spawned.ent_entry, (
        ProjectWindowEntryComponent {
            path: entry.path.clone(),
        },
        ButtonComponent::default(),
    ));

    let key = FoldoutKey::ProjectEntry { path: get_potential_asset_key(&entry.path, open_project) };
    let is_expanded = foldout_res.is_expanded(&key);
    let foldout = (!entry.children.is_empty()).then(|| (key, is_expanded));

    spawn_foldout_arrow(ec.as_mut(), spawned.ent_entry, spawned.ent_entry_name, 0.0, foldout, material_panel, material_text, font);

    return_if!(!is_expanded);

    for entry in &entry.children {
        spawn_project_window_entries(
            ec.as_mut(),
            material_text,
            material_panel,
            font,
            open_project,
            foldout_res,
            spawned.ent_children_container,
            entry,
        );
    }
}
