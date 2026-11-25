use std::{
    ffi::{OsStr, OsString},
    path::{Path, PathBuf},
};

use crate::{
    features::{
        clipboard::{EditorClipboard, EditorClipboardResource},
        game_window::GameViewFocusResource,
        generic_dropdown::{GenericDropdownResource, select_generic_dropdown_system},
        input_field::{
            SelectedInputFieldResource, is_editor_ctrl_hotkey_just_pressed, is_editor_hotkey_just_pressed,
            update_selected_input_field_text_system,
        },
        inspector_window::{
            data::{InspectedAssetResource, InspectedEntityResource, OpenedEntitiesAssetResource},
            systems::save_simulated_entities_to_asset_system,
        },
        project_reload::ProjectReloadResource,
        project_window_create::ProjectEntryCreateMenuResource,
        project_window_rename::{
            ProjectEntryRenameResource, get_entry_dir, get_non_clashing_path, split_entry_name, start_project_entry_rename_system,
        },
        world_preload::SimulatedWorldResource,
    },
    *,
};

// todo: bundle registration params
pub fn register_feature(mut data: WorldDataMut, mut behavior: WorldBehaviorBuilderMut, _render: &mut RenderFeatureRegistry) {
    data.resources_mut().insert(SelectedFileResource::default());
    data.resources_mut().insert(ProjectEntryContextMenuResource::default());

    let mut update = behavior.get_mut(Schedule::Update);

    update
        .group(SYSTEM_GROUP)
        .insert_child_system(select_file_system)
        .insert_child_system(update_project_entry_selection_system)
        .insert_child_system(open_project_entry_context_menu_system)
        .insert_child_system(apply_project_entry_context_menu_system)
        .insert_child_system(delete_selected_file_hotkey_system)
        .insert_child_system(rename_selected_file_hotkey_system)
        .insert_child_system(copy_selected_file_hotkey_system)
        .insert_child_system(paste_files_hotkey_system);

    update
        .order_system(check_ui_interaction_system)
        .before_system(select_file_system)
        .before_system(update_project_entry_selection_system);

    // a field deselected by Enter/Escape this frame still swallows the key
    update
        .order_system(delete_selected_file_hotkey_system)
        .before_system(update_selected_input_field_text_system);

    update
        .order_system(rename_selected_file_hotkey_system)
        .before_system(update_selected_input_field_text_system);

    update
        .order_system(copy_selected_file_hotkey_system)
        .before_system(update_selected_input_field_text_system);

    update
        .order_system(paste_files_hotkey_system)
        .before_system(update_selected_input_field_text_system);

    update
        .order_system(delete_selected_file_hotkey_system)
        .before_system(save_simulated_entities_to_asset_system);

    update
        .order_system(rename_selected_file_hotkey_system)
        .before_system(start_project_entry_rename_system);

    update
        .order_system(select_generic_dropdown_system)
        .before_system(open_project_entry_context_menu_system)
        .before_system(apply_project_entry_context_menu_system);
}

#[derive(Resource, Clone, Default)]
pub struct SelectedFileResource {
    pub path: OsString,
    pub potential_asset_key: String,
}

#[derive(Event, Copy, Clone, Default)]
pub struct FileSelectedEvent;

#[derive(Event, Copy, Clone, Default)]
pub struct FileOpenedEvent;

pub fn select_file_system(
    entry_q: WorldQuery<(&ProjectWindowEntryComponent, &ButtonComponent)>,
    open_project: Res<OpenProjectResource>,
    mut selected_file_res: ResMut<SelectedFileResource>,
    mut select_evt: EvtMut<FileSelectedEvent>,
    mut open_evt: EvtMut<FileOpenedEvent>,
) {
    let Some((entry_c, button_c)) = entry_q.iter().find(|(_, b)| b.was_clicked_this_frame) else {
        return;
    };

    selected_file_res.path = entry_c.path.clone();
    selected_file_res.potential_asset_key = get_potential_asset_key(&entry_c.path, &open_project);
    select_evt.push(FileSelectedEvent);

    if button_c.was_double_clicked_this_frame {
        open_evt.push(FileOpenedEvent);
    }
}

pub fn get_asset_key(path: &OsStr, open_project: &OpenProjectResource) -> Option<String> {
    let assets_dir_path = open_project.dir_path.to_string() + PROJECT_ASSETS_SUBPATH;
    let key = path.to_str()?.strip_prefix(&assets_dir_path)?;
    Some(key.replace("\\", "/"))
}

pub fn get_potential_asset_key(path: &OsStr, open_project: &OpenProjectResource) -> String {
    get_asset_key(path, open_project).unwrap_or_else(|| path.to_str().unwrap_or("").to_string())
}

#[derive(Clone, Copy)]
enum ProjectEntryContextAction {
    Create,
    ShowInExplorer,
    CopyAssetKey,
    Copy,
    Paste,
    Rename,
    Delete,
}

impl ProjectEntryContextAction {
    fn label(self) -> &'static str {
        match self {
            Self::Create => "Create",
            Self::ShowInExplorer => "Show in explorer",
            Self::CopyAssetKey => "Copy asset key",
            Self::Copy => "Copy",
            Self::Paste => "Paste",
            Self::Rename => "Rename",
            Self::Delete => "Delete",
        }
    }
}

#[derive(Resource, Default)]
pub struct ProjectEntryContextMenuResource {
    request_id: u64,
    path: OsString,
    position: Vec2<f32>,
    actions: Vec<ProjectEntryContextAction>,
}

pub fn open_project_entry_context_menu_system(
    input_res: Res<InputResource>,
    entry_q: WorldQuery<(&ProjectWindowEntryComponent, &ButtonComponent)>,
    empty_area_q: WorldQuery<&ButtonComponent, WithFilter<ProjectWindowEmptyAreaComponent>>,
    open_project: Res<OpenProjectResource>,
    simulated_world: Res<SimulatedWorldResource>,
    clipboard: Res<EditorClipboardResource>,
    mut generic_dropdown_res: ResMut<GenericDropdownResource>,
    mut context_menu_res: ResMut<ProjectEntryContextMenuResource>,
) {
    let has_clipboard_files = clipboard.files().is_some();

    let (path, actions) = if let Some((entry_c, _)) = entry_q.iter().find(|(_, b)| b.was_right_clicked_this_frame) {
        let mut actions = Vec::new();
        let key = get_asset_key(&entry_c.path, &open_project);

        if key.is_some() {
            actions.push(ProjectEntryContextAction::Create);
        }

        actions.push(ProjectEntryContextAction::ShowInExplorer);

        if let Some(key) = key {
            actions.push(ProjectEntryContextAction::CopyAssetKey);

            if !key.is_empty() {
                actions.push(ProjectEntryContextAction::Copy);
            }

            if has_clipboard_files {
                actions.push(ProjectEntryContextAction::Paste);
            }

            if !key.is_empty() {
                actions.push(ProjectEntryContextAction::Rename);
                actions.push(ProjectEntryContextAction::Delete);
            }
        }

        (entry_c.path.clone(), actions)
    } else if empty_area_q.iter().any(|b| b.was_right_clicked_this_frame) {
        let mut actions = vec![ProjectEntryContextAction::Create];

        if has_clipboard_files {
            actions.push(ProjectEntryContextAction::Paste);
        }

        (get_assets_dir_path(&open_project), actions)
    } else {
        return;
    };

    // todo: support file changes in play mode
    let actions = actions
        .into_iter()
        .filter(|a| {
            !simulated_world.is_playing()
                || matches!(
                    a,
                    ProjectEntryContextAction::ShowInExplorer
                        | ProjectEntryContextAction::CopyAssetKey
                        | ProjectEntryContextAction::Copy
                )
        })
        .collect::<Vec<_>>();
    return_if!(actions.is_empty());

    let position = Vec2::from_array(input_res.mouse.position.map(|v| v as f32));
    context_menu_res.request_id =
        generic_dropdown_res.request(actions.iter().map(|a| FfiString::from(a.label())).collect(), position);
    context_menu_res.path = path;
    context_menu_res.position = position;
    context_menu_res.actions = actions;
}

pub fn apply_project_entry_context_menu_system(
    mut generic_dropdown_res: ResMut<GenericDropdownResource>,
    open_project: Res<OpenProjectResource>,
    mut context_menu_res: ResMut<ProjectEntryContextMenuResource>,
    mut create_menu_res: ResMut<ProjectEntryCreateMenuResource>,
    mut selected_file: ResMut<SelectedFileResource>,
    mut inspected_asset: ResMut<InspectedAssetResource>,
    mut opened_asset: ResMut<OpenedEntitiesAssetResource>,
    mut inspected_entity: ResMut<InspectedEntityResource>,
    mut simulated_world: ResMut<SimulatedWorldResource>,
    mut rename_res: ResMut<ProjectEntryRenameResource>,
    mut clipboard: ResMut<EditorClipboardResource>,
    mut project_reload: ResMut<ProjectReloadResource>,
    mut select_evt: EvtMut<FileSelectedEvent>,
) {
    let context_menu = &mut *context_menu_res;

    let Some(idx) = generic_dropdown_res.take_selected(&mut context_menu.request_id) else {
        return;
    };

    match context_menu.actions[idx as usize] {
        ProjectEntryContextAction::Create => {
            let path = Path::new(&context_menu.path);
            let dir = if path.is_dir() {
                Some(path.to_path_buf())
            } else {
                get_entry_dir(path)
            };

            match dir {
                Some(dir) => create_menu_res.request(&mut generic_dropdown_res, dir, context_menu.position),
                None => eprintln!("failed to create next to {:?}. non-utf8 path", path),
            }
        }
        ProjectEntryContextAction::ShowInExplorer => reveal_in_file_explorer(Path::new(&context_menu.path)),
        ProjectEntryContextAction::CopyAssetKey => {
            if let Some(key) = get_asset_key(&context_menu.path, &open_project) {
                copy_to_clipboard(&key);
            }
        }
        ProjectEntryContextAction::Copy => clipboard.0 = Some(EditorClipboard::Files(vec![PathBuf::from(&context_menu.path)])),
        ProjectEntryContextAction::Paste => {
            return_if_not!(Some(dest_dir) = get_paste_dir(Path::new(&context_menu.path)));
            paste_clipboard_files(
                &clipboard,
                &dest_dir,
                &open_project,
                &mut selected_file,
                &mut select_evt,
                &mut project_reload,
            );
        }
        ProjectEntryContextAction::Rename => rename_res.request(context_menu.path.clone()),
        ProjectEntryContextAction::Delete => delete_project_entry(
            Path::new(&context_menu.path),
            &open_project,
            &mut selected_file,
            &mut inspected_asset,
            &mut opened_asset,
            &mut inspected_entity,
            &mut simulated_world,
        ),
    }
}

// the same entries the context menu offers Rename and Delete for
fn is_modifiable_project_entry(path: &OsStr, open_project: &OpenProjectResource, simulated_world: &SimulatedWorldResource) -> bool {
    get_asset_key(path, open_project).is_some_and(|key| !key.is_empty()) && !simulated_world.is_playing()
}

pub fn delete_selected_file_hotkey_system(
    input_res: Res<InputResource>,
    selected_input: Res<SelectedInputFieldResource>,
    game_focus: Res<GameViewFocusResource>,
    open_project: Res<OpenProjectResource>,
    mut selected_file: ResMut<SelectedFileResource>,
    mut inspected_asset: ResMut<InspectedAssetResource>,
    mut opened_asset: ResMut<OpenedEntitiesAssetResource>,
    mut inspected_entity: ResMut<InspectedEntityResource>,
    mut simulated_world: ResMut<SimulatedWorldResource>,
) {
    if !is_editor_hotkey_just_pressed(&input_res, &selected_input, &game_focus, KeyCode::Delete)
        || !is_modifiable_project_entry(&selected_file.path, &open_project, &simulated_world)
    {
        return;
    }

    let path = PathBuf::from(&selected_file.path);
    delete_project_entry(
        &path,
        &open_project,
        &mut selected_file,
        &mut inspected_asset,
        &mut opened_asset,
        &mut inspected_entity,
        &mut simulated_world,
    );
}

pub fn rename_selected_file_hotkey_system(
    input_res: Res<InputResource>,
    selected_input: Res<SelectedInputFieldResource>,
    game_focus: Res<GameViewFocusResource>,
    open_project: Res<OpenProjectResource>,
    selected_file: Res<SelectedFileResource>,
    simulated_world: Res<SimulatedWorldResource>,
    mut rename_res: ResMut<ProjectEntryRenameResource>,
) {
    if !is_editor_hotkey_just_pressed(&input_res, &selected_input, &game_focus, KeyCode::F2)
        || !is_modifiable_project_entry(&selected_file.path, &open_project, &simulated_world)
    {
        return;
    }

    rename_res.request(selected_file.path.clone());
}

pub fn copy_selected_file_hotkey_system(
    input_res: Res<InputResource>,
    selected_input: Res<SelectedInputFieldResource>,
    game_focus: Res<GameViewFocusResource>,
    animation_window: Res<crate::features::animation_window::AnimationWindowResource>,
    open_project: Res<OpenProjectResource>,
    selected_file: Res<SelectedFileResource>,
    mut clipboard: ResMut<EditorClipboardResource>,
) {
    return_if!(animation_window.is_focused);
    return_if!(!is_editor_ctrl_hotkey_just_pressed(
        &input_res,
        &selected_input,
        &game_focus,
        KeyCode::KeyC
    ));
    return_if!(!get_asset_key(&selected_file.path, &open_project).is_some_and(|key| !key.is_empty()));

    clipboard.0 = Some(EditorClipboard::Files(vec![PathBuf::from(&selected_file.path)]));
}

pub fn paste_files_hotkey_system(
    input_res: Res<InputResource>,
    selected_input: Res<SelectedInputFieldResource>,
    game_focus: Res<GameViewFocusResource>,
    animation_window: Res<crate::features::animation_window::AnimationWindowResource>,
    open_project: Res<OpenProjectResource>,
    simulated_world: Res<SimulatedWorldResource>,
    inspected_entity: Res<InspectedEntityResource>,
    clipboard: Res<EditorClipboardResource>,
    mut selected_file: ResMut<SelectedFileResource>,
    mut select_evt: EvtMut<FileSelectedEvent>,
    mut project_reload: ResMut<ProjectReloadResource>,
) {
    return_if!(animation_window.is_focused);
    return_if!(!is_editor_ctrl_hotkey_just_pressed(
        &input_res,
        &selected_input,
        &game_focus,
        KeyCode::KeyV
    ));
    return_if!(clipboard.files().is_none() || simulated_world.is_playing());

    let dest_dir = if get_asset_key(&selected_file.path, &open_project).is_some() {
        get_paste_dir(Path::new(&selected_file.path))
    } else if selected_file.path.is_empty() && inspected_entity.selected_entity == EntityId::EMPTY {
        Some(PathBuf::from(get_assets_dir_path(&open_project)))
    } else {
        None
    };
    return_if_not!(Some(dest_dir) = dest_dir);

    paste_clipboard_files(
        &clipboard,
        &dest_dir,
        &open_project,
        &mut selected_file,
        &mut select_evt,
        &mut project_reload,
    );
}

fn get_assets_dir_path(open_project: &OpenProjectResource) -> OsString {
    OsString::from(open_project.dir_path.to_string() + PROJECT_ASSETS_SUBPATH)
}

// a folder receives the pasted entries, a file gets them pasted next to it
fn get_paste_dir(path: &Path) -> Option<PathBuf> {
    match path.is_dir() {
        true => Some(path.to_path_buf()),
        false => get_entry_dir(path),
    }
}

fn paste_clipboard_files(
    clipboard: &EditorClipboardResource,
    dest_dir: &Path,
    open_project: &OpenProjectResource,
    selected_file: &mut SelectedFileResource,
    select_evt: &mut EvtMut<FileSelectedEvent>,
    project_reload: &mut ProjectReloadResource,
) {
    return_if_not!(Some(sources) = clipboard.files());

    let mut last_pasted = None;

    for source in sources {
        if let Some(pasted) = paste_project_entry(source, dest_dir) {
            last_pasted = Some(pasted);
        }
    }

    return_if_not!(Some(last_pasted) = last_pasted);

    let path = last_pasted.into_os_string();
    selected_file.potential_asset_key = get_potential_asset_key(&path, open_project);
    selected_file.path = path;
    select_evt.push(FileSelectedEvent);

    project_reload.request_assets_reload();
}

fn paste_project_entry(source: &Path, dest_dir: &Path) -> Option<PathBuf> {
    if !source.exists() {
        eprintln!("failed to paste {:?}. it no longer exists", source);
        return None;
    }

    if source.is_dir() && dest_dir.starts_with(source) {
        eprintln!(
            "failed to paste {:?} into {:?}. a folder cannot be pasted into itself",
            source, dest_dir
        );
        return None;
    }

    let (stem, extension) = split_entry_name(source);
    let dest = get_non_clashing_path(dest_dir, &stem, extension.as_deref());

    let result = match source.is_dir() {
        true => copy_dir_recursively(source, &dest),
        false => std::fs::copy(source, &dest).map(|_| ()),
    };

    if let Err(err) = result {
        eprintln!("failed to paste {:?} to {:?}. {}", source, dest, err);
    }

    dest.exists().then_some(dest)
}

fn copy_dir_recursively(source: &Path, dest: &Path) -> std::io::Result<()> {
    std::fs::create_dir(dest)?;

    for entry in std::fs::read_dir(source)? {
        let entry = entry?;
        let entry_dest = dest.join(entry.file_name());

        match entry.file_type()?.is_dir() {
            true => copy_dir_recursively(&entry.path(), &entry_dest)?,
            false => _ = std::fs::copy(entry.path(), &entry_dest)?,
        }
    }

    Ok(())
}

fn delete_project_entry(
    path: &Path,
    open_project: &OpenProjectResource,
    selected_file: &mut SelectedFileResource,
    inspected_asset: &mut InspectedAssetResource,
    opened_asset: &mut OpenedEntitiesAssetResource,
    inspected_entity: &mut InspectedEntityResource,
    simulated_world: &mut SimulatedWorldResource,
) {
    let result = if path.is_dir() {
        std::fs::remove_dir_all(path)
    } else {
        std::fs::remove_file(path)
    };
    if let Err(err) = result {
        eprintln!("failed to delete {:?}. {}", path, err);
    }

    if !path.exists() {
        apply_project_path_change(
            path,
            None,
            open_project,
            selected_file,
            inspected_asset,
            opened_asset,
            inspected_entity,
            simulated_world,
        );
    }
}

enum PathChange {
    Unaffected,
    Removed,
    Moved(OsString),
}

fn get_path_change(path: &OsStr, old_path: &Path, new_path: Option<&Path>) -> PathChange {
    let Ok(rest) = Path::new(path).strip_prefix(old_path) else {
        return PathChange::Unaffected;
    };

    match new_path {
        None => PathChange::Removed,
        Some(new_path) if rest.as_os_str().is_empty() => PathChange::Moved(new_path.into()),
        Some(new_path) => PathChange::Moved(new_path.join(rest).into()),
    }
}

pub fn apply_project_path_change(
    old_path: &Path,
    new_path: Option<&Path>,
    open_project: &OpenProjectResource,
    selected_file: &mut SelectedFileResource,
    inspected_asset: &mut InspectedAssetResource,
    opened_asset: &mut OpenedEntitiesAssetResource,
    inspected_entity: &mut InspectedEntityResource,
    simulated_world: &mut SimulatedWorldResource,
) {
    match get_path_change(&selected_file.path, old_path, new_path) {
        PathChange::Unaffected => {}
        PathChange::Removed => *selected_file = SelectedFileResource::default(),
        PathChange::Moved(path) => {
            *selected_file = SelectedFileResource {
                potential_asset_key: get_potential_asset_key(&path, open_project),
                path,
            };
        }
    }

    match get_path_change(&inspected_asset.path, old_path, new_path) {
        PathChange::Unaffected => {}
        PathChange::Removed => *inspected_asset = InspectedAssetResource::default(),
        PathChange::Moved(path) => {
            *inspected_asset = InspectedAssetResource {
                asset_key: get_potential_asset_key(&path, open_project),
                path,
                edited: None,
            }
        }
    }

    let is_opened_asset_removed = match get_path_change(&opened_asset.path, old_path, new_path) {
        PathChange::Unaffected => false,
        PathChange::Removed => true,
        PathChange::Moved(path) => {
            opened_asset.asset_key = get_potential_asset_key(&path, open_project);
            opened_asset.path = path;
            false
        }
    };

    if is_opened_asset_removed {
        *opened_asset = OpenedEntitiesAssetResource::default();
        *inspected_entity = InspectedEntityResource::default();
    }

    return_if_not!(Some(simulated_world) = &mut simulated_world.0);

    let (mut res, mut ent, _) = simulated_world.data.as_mut().into_tuple_mut();

    if is_opened_asset_removed {
        for entity in ent.query::<EntityId>().iter().collect::<Vec<_>>() {
            ent.destroy_entity(entity);
        }
    }

    return_if_not!(Some(old_key) = get_asset_key(old_path.as_os_str(), open_project));
    let new_key = new_path.and_then(|p| get_asset_key(p.as_os_str(), open_project));
    let new_key = new_key.as_deref();

    rekey_registered_assets::<StandardTexture>(res.as_mut(), &old_key, new_key);
    rekey_registered_assets::<Material>(res.as_mut(), &old_key, new_key);
    rekey_registered_assets::<MaterialTemplate>(res.as_mut(), &old_key, new_key);
    rekey_registered_assets::<Shader>(res.as_mut(), &old_key, new_key);
    rekey_registered_assets::<StandardMesh>(res.as_mut(), &old_key, new_key);
    rekey_registered_assets::<Font>(res.as_mut(), &old_key, new_key);
    rekey_registered_assets::<AudioClip>(res.as_mut(), &old_key, new_key);
    rekey_registered_assets::<AnimationClip>(res.as_mut(), &old_key, new_key);
    rekey_registered_assets::<Prefab>(res.as_mut(), &old_key, new_key);
    rekey_registered_assets::<Scene>(res.as_mut(), &old_key, new_key);
}

fn rekey_registered_assets<T: 'static + Send + Sync>(res: ResourcesHolderMut, old_key: &str, new_key: Option<&str>) {
    return_if_not!(Some(storage) = res.into_get_mut::<AssetStorageResource<T>>());

    let keys = storage
        .get_all()
        .filter_map(|(_, key)| key)
        .filter(|key| {
            key.strip_prefix(old_key)
                .is_some_and(|rest| rest.is_empty() || rest.starts_with('/'))
        })
        .map(String::from)
        .collect::<Vec<_>>();

    for key in keys {
        let handle = storage.get_registered(&key).cloned();
        storage.unregister(&key);

        if let (Some(handle), Some(new_key)) = (handle, new_key) {
            storage.register(FfiString::from_string(format!("{new_key}{}", &key[old_key.len()..])), handle);
        }
    }
}

pub fn update_project_entry_selection_system(
    inspected_file: Res<SelectedFileResource>,
    mut entry_q: WorldQuery<(&ProjectWindowEntryComponent, &mut ImageComponent)>,
) {
    for (entry_c, image_c) in entry_q.iter_mut() {
        if entry_c.path == inspected_file.path {
            image_c.color = Vec4::from_array(const { parse_color_rgba_f32("#7d90e6ff").unwrap() });
        } else {
            image_c.color = Vec4::from_array(const { parse_color_rgba_f32("#00000000").unwrap() });
        }
    }
}
