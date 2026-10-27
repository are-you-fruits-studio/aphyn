use std::{
    ffi::OsString,
    path::{Path, PathBuf},
};

use crate::{
    features::{
        foldout::FOLDOUT_WIDTH,
        generic_dropdown::select_generic_dropdown_system,
        input_field::{
            InputFieldComponent, InputFieldSelectionChangedEvent, SelectedInputFieldResource, apply_input_field_context_menu_system,
            select_input_field_system, update_selected_input_field_text_system,
        },
        inspector_window::{
            data::{InspectedAssetResource, InspectedEntityResource, OpenedEntitiesAssetResource},
            systems::save_simulated_entities_to_asset_system,
            utils::entries::spawn_input_area_ent,
        },
        project_window_entries::update_project_window_content_system,
        project_window_selection::{SelectedFileResource, apply_project_entry_context_menu_system, apply_project_path_change},
        world_preload::SimulatedWorldResource,
    },
    *,
};

pub fn register_feature(mut world: WorldBuilderMut) {
    world.data_mut().resources_mut().insert(ProjectEntryRenameResource::default());

    let mut behavior = world.behavior_mut();
    let mut update = behavior.get_mut(Schedule::Update);

    update
        .group(SYSTEM_GROUP)
        .insert_child_system(finish_project_entry_rename_system)
        .insert_child_system(start_project_entry_rename_system);

    update
        .order_system(select_input_field_system)
        .before_system(finish_project_entry_rename_system);

    update
        .order_system(update_selected_input_field_text_system)
        .before_system(finish_project_entry_rename_system);

    update
        .order_system(apply_input_field_context_menu_system)
        .before_system(finish_project_entry_rename_system);

    update
        .order_system(select_generic_dropdown_system)
        .before_system(finish_project_entry_rename_system);

    update
        .order_system(finish_project_entry_rename_system)
        .before_system(save_simulated_entities_to_asset_system);

    update
        .order_system(finish_project_entry_rename_system)
        .before_system(start_project_entry_rename_system);

    update
        .order_system(apply_project_entry_context_menu_system)
        .before_system(start_project_entry_rename_system);

    update
        .order_system(update_project_window_content_system)
        .before_system(start_project_entry_rename_system);
}

#[derive(Resource, Default)]
pub struct ProjectEntryRenameResource {
    pending: Option<OsString>,
    active: Option<ActiveRename>,
}

impl ProjectEntryRenameResource {
    pub fn request(&mut self, path: OsString) {
        self.pending = Some(path);
    }
}

struct ActiveRename {
    path: OsString,
    name_text: EntityId,
    input_field: EntityId,
}

pub fn start_project_entry_rename_system(
    mut ent: EntitiesHolderMut,
    assets: Res<StandardAssetsResource>,
    mut rename_res: ResMut<ProjectEntryRenameResource>,
    mut selected_input: ResMut<SelectedInputFieldResource>,
    mut selection_changed_evt: EvtMut<InputFieldSelectionChangedEvent>,
) {
    return_if!(rename_res.active.is_some());
    return_if_not!(Some(path) = rename_res.pending.clone());

    if !Path::new(&path).exists() {
        rename_res.pending = None;
        return;
    }

    // entries for a freshly created path only appear after the next project window rescan
    let Some(ent_entry) = ent
        .query::<(EntityId, &ProjectWindowEntryComponent)>()
        .iter()
        .find(|(_, entry_c)| entry_c.path == path)
        .map(|(e, _)| e)
    else {
        return;
    };

    rename_res.pending = None;

    let Some(name_text) = ent
        .query::<(EntityId, &ChildComponent, &TextComponent)>()
        .iter()
        .find(|(_, child_c, _)| child_c.parent == ent_entry)
        .map(|(e, _, _)| e)
    else {
        return;
    };

    let (stem, _) = split_entry_name(Path::new(&path));

    if let Some(text_c) = ent.get_component_mut::<TextComponent>(name_text) {
        text_c.text.clear();
    }

    let input_field = spawn_input_area_ent(
        ent.as_mut(),
        ent_entry,
        stem.as_str().into(),
        assets.material_panel.clone(),
        assets.material_text.clone(),
        assets.font.clone(),
    );

    // in front of the entry button, so clicks on the input don't hit the entry and deselect the input
    if let Some(local_rect_c) = ent.get_component_mut::<LocalRectComponent>(input_field) {
        local_rect_c.z = -1.0;
        local_rect_c.parent_padding_min.x = UiVal::px(FOLDOUT_WIDTH);
    }

    selected_input.selected = input_field;
    selection_changed_evt.push(InputFieldSelectionChangedEvent);

    rename_res.active = Some(ActiveRename {
        path,
        name_text,
        input_field,
    });
}

pub fn finish_project_entry_rename_system(
    mut ent: EntitiesHolderMut,
    input_res: Res<InputResource>,
    open_project: Res<OpenProjectResource>,
    mut rename_res: ResMut<ProjectEntryRenameResource>,
    mut selected_input: ResMut<SelectedInputFieldResource>,
    mut selection_changed_evt: EvtMut<InputFieldSelectionChangedEvent>,
    mut selected_file: ResMut<SelectedFileResource>,
    mut inspected_asset: ResMut<InspectedAssetResource>,
    mut opened_asset: ResMut<OpenedEntitiesAssetResource>,
    mut inspected_entity: ResMut<InspectedEntityResource>,
    mut simulated_world: ResMut<SimulatedWorldResource>,
) {
    return_if_not!(Some(active) = &rename_res.active);

    let is_alive = ent.get_component::<InputFieldComponent>(active.input_field).is_some();
    let is_cancelled = !is_alive || input_res.keyboard.is_just_pressed(KeyCode::Escape);
    let is_committed = input_res.keyboard.is_just_pressed(KeyCode::Enter)
        || input_res.keyboard.is_just_pressed(KeyCode::NumpadEnter)
        || selected_input.selected != active.input_field;

    return_if!(!is_cancelled && !is_committed);

    let active = rename_res.active.take().unwrap();
    let old_path = PathBuf::from(&active.path);

    let typed_stem = (!is_cancelled)
        .then(|| get_input_field_text(ent.as_ref(), active.input_field))
        .flatten();
    let new_path = typed_stem.and_then(|stem| rename_project_entry(&old_path, &stem));

    if let Some(new_path) = &new_path {
        // todo: references to the renamed asset key(s) inside other .asset files and loaded assets are not rewritten and will break
        apply_project_path_change(
            &old_path,
            Some(new_path),
            &open_project,
            &mut selected_file,
            &mut inspected_asset,
            &mut opened_asset,
            &mut inspected_entity,
            &mut simulated_world,
        );
    }

    if selected_input.selected == active.input_field {
        selected_input.selected = EntityId::EMPTY;
        selection_changed_evt.push(InputFieldSelectionChangedEvent);
    }

    destroy_input_field(ent.as_mut(), active.input_field);

    let shown_path = new_path.unwrap_or(old_path);
    if let Some(text_c) = ent.get_component_mut::<TextComponent>(active.name_text) {
        text_c.text = shown_path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default()
            .into();
    }
}

fn get_input_field_text(ent: EntitiesHolderRef, input_field: EntityId) -> Option<String> {
    let input_field_c = ent.get_component::<InputFieldComponent>(input_field)?;
    let text_c = ent.get_component::<TextComponent>(input_field_c.text)?;
    Some(text_c.text.as_str().chars().filter(|c| !c.is_control()).collect())
}

fn destroy_input_field(mut ent: EntitiesHolderMut, input_field: EntityId) {
    let Some(&InputFieldComponent { text, selection_border }) = ent.get_component::<InputFieldComponent>(input_field) else {
        return;
    };

    let background = ent.get_component::<ChildComponent>(text).map(|c| c.parent);

    destroy_entity_and_children(ent.as_mut(), input_field);
    for e in [Some(text), Some(selection_border), background].into_iter().flatten() {
        ent.destroy_entity(e);
    }
}

fn split_entry_name(path: &Path) -> (String, Option<String>) {
    let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();

    if path.is_dir() {
        return (name, None);
    }

    let stem = path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or(name);
    let extension = path.extension().map(|e| e.to_string_lossy().into_owned());
    (stem, extension)
}

fn join_file_name(stem: &str, extension: Option<&str>) -> String {
    match extension {
        Some(extension) => format!("{stem}.{extension}"),
        None => stem.to_string(),
    }
}

fn rename_project_entry(old_path: &Path, new_stem: &str) -> Option<PathBuf> {
    let old_name = old_path.file_name()?.to_str()?;
    let (_, extension) = split_entry_name(old_path);
    let new_name = join_file_name(new_stem, extension.as_deref());

    if new_name == old_name {
        return None;
    }

    if !is_valid_file_name(&new_name) {
        eprintln!("failed to rename {:?}. invalid name {:?}", old_path, new_name);
        return None;
    }

    let Some(dir) = get_entry_dir(old_path) else {
        eprintln!("failed to rename {:?}. non-utf8 path", old_path);
        return None;
    };

    let is_case_only_change = cfg!(windows) && new_name.to_lowercase() == old_name.to_lowercase();

    let new_path = match is_case_only_change {
        true => dir.join(&new_name),
        false => get_non_clashing_path(&dir, new_stem, extension.as_deref()),
    };

    if let Err(err) = std::fs::rename(old_path, &new_path) {
        eprintln!("failed to rename {:?} to {:?}. {}", old_path, new_path, err);
        return None;
    }

    Some(new_path)
}

// keeps the separators of the scanned path, so a joined sibling compares equal to the rescanned entry path
pub fn get_entry_dir(path: &Path) -> Option<PathBuf> {
    let path_str = path.to_str()?;
    let name = path.file_name()?.to_str()?;
    Some(PathBuf::from(path_str.strip_suffix(name)?))
}

pub fn get_non_clashing_path(dir: &Path, stem: &str, extension: Option<&str>) -> PathBuf {
    let mut path = dir.join(join_file_name(stem, extension));
    let mut idx = 1;

    while path.symlink_metadata().is_ok() {
        path = dir.join(join_file_name(&format!("{stem} ({idx})"), extension));
        idx += 1;
    }

    path
}

pub fn is_valid_file_name(name: &str) -> bool {
    const RESERVED: [&str; 22] = [
        "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8", "COM9", "LPT1", "LPT2", "LPT3",
        "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
    ];

    let stem = name.split('.').next().unwrap_or("").trim_end();

    !name.is_empty()
        && name != "."
        && name != ".."
        && !name.ends_with(['.', ' '])
        && !name
            .chars()
            .any(|c| c.is_control() || matches!(c, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*'))
        && !RESERVED.iter().any(|r| r.eq_ignore_ascii_case(stem))
}
