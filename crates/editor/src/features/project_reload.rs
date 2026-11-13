use std::{
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread::JoinHandle,
    time::{Duration, Instant},
};

use notify::{EventKind, RecursiveMode, Watcher as _};

use crate::{
    PROJECT_ASSETS_SUBPATH, SYSTEM_GROUP, app_building,
    features::{
        inspector_window::{
            data::{InspectedEntityResource, OpenProjectResource, OpenedEntitiesAssetResource},
            systems::{open_entities_asset, select_entity_system, update_hierarchy_window_system, update_inspector_window_system},
        },
        play_mode::toggle_play_mode_system,
        project_window_selection::FileSelectedEvent,
        world_preload::{
            SimulatedWorldResource, SimulationMode, preload_assets_into_simulated_world_system, rebuild_simulated_world,
            tick_simulated_world_system,
        },
    },
    *,
};

// asset events right after focus loss are most likely the editor's own late-delivered writes
const UNFOCUSED_GRACE_PERIOD: Duration = Duration::from_millis(250);

// todo: bundle registration params
pub fn register_feature(mut data: WorldDataMut, mut behavior: WorldBehaviorBuilderMut, _render: &mut RenderFeatureRegistry) {
    data.resources_mut().insert(ProjectReloadResource::default());

    let mut update = behavior.get_mut(Schedule::Update);

    update
        .group(SYSTEM_GROUP)
        .insert_child_system(track_window_focus_system)
        .insert_child_system(start_scripts_build_system)
        .insert_child_system(finish_scripts_build_system)
        .insert_child_system(apply_project_reload_system);

    update
        .order_system(track_window_focus_system)
        .before_system(start_scripts_build_system)
        .before_system(finish_scripts_build_system)
        .before_system(preload_assets_into_simulated_world_system)
        .before_system(apply_project_reload_system)
        .before_system(toggle_play_mode_system);

    update
        .order_system(apply_project_reload_system)
        .before_system(tick_simulated_world_system);
    update
        .order_system(apply_project_reload_system)
        .before_system(select_entity_system);
    update
        .order_system(apply_project_reload_system)
        .before_system(update_hierarchy_window_system);
    update
        .order_system(apply_project_reload_system)
        .before_system(update_inspector_window_system);
}

#[derive(Resource)]
pub struct ProjectReloadResource {
    build: Option<JoinHandle<Result<(), String>>>,
    is_build_requested: bool,
    is_initial_build_done: bool,
    is_scripts_reload_pending: bool,
    is_assets_reload_pending: bool,
    assets_watcher: Option<AssetsWatcher>,
}

impl Default for ProjectReloadResource {
    fn default() -> Self {
        Self {
            build: None,
            is_build_requested: true,
            is_initial_build_done: false,
            is_scripts_reload_pending: false,
            is_assets_reload_pending: false,
            assets_watcher: None,
        }
    }
}

impl ProjectReloadResource {
    pub fn is_initial_build_done(&self) -> bool {
        self.is_initial_build_done
    }

    pub fn is_reload_pending(&self) -> bool {
        self.is_scripts_reload_pending || self.is_assets_reload_pending
    }

    // returns whether the scripts library has to be reloaded
    pub fn take_pending(&mut self) -> bool {
        self.is_assets_reload_pending = false;
        std::mem::take(&mut self.is_scripts_reload_pending)
    }
}

struct AssetsWatcher {
    path: PathBuf,
    is_dirty: Arc<AtomicBool>,
    unfocused_since: Arc<Mutex<Option<Instant>>>,
    _watcher: Option<Mutex<notify::RecommendedWatcher>>,
}

impl AssetsWatcher {
    fn new(path: PathBuf) -> Self {
        let is_dirty = Arc::new(AtomicBool::new(false));
        let unfocused_since = Arc::new(Mutex::new(None::<Instant>));

        let is_dirty_handler = is_dirty.clone();
        let unfocused_since_handler = unfocused_since.clone();

        let watcher = notify::recommended_watcher(move |event: notify::Result<notify::Event>| {
            if let Ok(event) = &event
                && let EventKind::Access(_) = event.kind
            {
                return;
            }

            let is_external = unfocused_since_handler
                .lock()
                .unwrap()
                .is_some_and(|since| since.elapsed() > UNFOCUSED_GRACE_PERIOD);

            if is_external {
                is_dirty_handler.store(true, Ordering::Relaxed);
            }
        })
        .and_then(|mut watcher| watcher.watch(&path, RecursiveMode::Recursive).map(|_| watcher));

        let watcher = match watcher {
            Ok(watcher) => Some(Mutex::new(watcher)),
            Err(err) => {
                eprintln!("failed to watch {:?}, assets won't be reloaded on focus. {}", path, err);
                None
            }
        };

        Self {
            path,
            is_dirty,
            unfocused_since,
            _watcher: watcher,
        }
    }

    fn set_focused(&self, is_focused: bool) {
        *self.unfocused_since.lock().unwrap() = (!is_focused).then(Instant::now);
    }

    fn take_dirty(&self) -> bool {
        self.is_dirty.swap(false, Ordering::Relaxed)
    }
}

fn track_window_focus_system(
    focus_changed_evt: Evt<WindowFocusChangedEvent>,
    open_project: Res<OpenProjectResource>,
    mut project_reload: ResMut<ProjectReloadResource>,
) {
    let assets_dir = PathBuf::from(open_project.dir_path.clone() + PROJECT_ASSETS_SUBPATH);

    if project_reload.assets_watcher.as_ref().is_none_or(|w| w.path != assets_dir) {
        project_reload.assets_watcher = Some(AssetsWatcher::new(assets_dir));
    }

    return_if_not!(Some(WindowFocusChangedEvent(is_focused)) = focus_changed_evt.iter().last().copied());

    let watcher = project_reload.assets_watcher.as_ref().unwrap();
    watcher.set_focused(is_focused);

    if is_focused {
        let is_assets_changed = watcher.take_dirty();

        project_reload.is_assets_reload_pending |= is_assets_changed;
        project_reload.is_build_requested = true;
    }
}

fn start_scripts_build_system(open_project: Res<OpenProjectResource>, mut project_reload: ResMut<ProjectReloadResource>) {
    return_if!(!project_reload.is_build_requested || project_reload.build.is_some());

    project_reload.is_build_requested = false;

    let project_dir = open_project.dir_path.clone();

    println!("scripts: building");

    project_reload.build = Some(std::thread::spawn(move || {
        app_building::build_scripts_lib(&project_dir).map_err(|err| err.to_string())
    }));
}

fn finish_scripts_build_system(
    open_project: Res<OpenProjectResource>,
    simulated_world: Res<SimulatedWorldResource>,
    mut project_reload: ResMut<ProjectReloadResource>,
) {
    return_if!(!project_reload.build.as_ref().is_some_and(|b| b.is_finished()));

    let result = project_reload.build.take().unwrap().join();

    match result {
        Ok(Ok(())) => (),
        Ok(Err(err)) => println!("scripts: build failed, keeping current library. {err}"),
        Err(_) => println!("scripts: build panicked, keeping current library"),
    }

    project_reload.is_initial_build_done = true;

    return_if_not!(Some(world) = &simulated_world.0);

    let lib_stamp = app_building::file_stamp(app_building::path_scripts_lib_src(&open_project.dir_path));

    if lib_stamp != world.library_stamp {
        project_reload.is_scripts_reload_pending = true;
    }
}

fn apply_project_reload_system(
    open_project: Res<OpenProjectResource>,
    render_api: Res<RenderApiResource>,
    mut project_reload: ResMut<ProjectReloadResource>,
    mut simulated_world: ResMut<SimulatedWorldResource>,
    mut opened_asset: ResMut<OpenedEntitiesAssetResource>,
    mut inspected_entity: ResMut<InspectedEntityResource>,
    mut file_selected_evt: EvtMut<FileSelectedEvent>,
) {
    return_if!(!project_reload.is_reload_pending());

    if simulated_world.0.is_none() {
        // the world is built from scratch anyway
        project_reload.take_pending();
        return;
    }

    // applied on Stop
    return_if!(simulated_world.is_playing());

    let reload_library = project_reload.take_pending();

    rebuild_simulated_world(
        &mut simulated_world,
        &mut inspected_entity,
        &open_project.dir_path,
        &render_api,
        SimulationMode::Edit,
        "",
        reload_library,
    );

    let opened = std::mem::take(&mut *opened_asset);
    let world = simulated_world.0.as_mut().unwrap();

    open_entities_asset(
        world,
        &mut opened_asset,
        &mut inspected_entity,
        &opened.asset_key,
        &opened.path,
    );

    file_selected_evt.push(FileSelectedEvent);

    println!("{} reloaded", if reload_library { "scripts and assets" } else { "assets" });
}
