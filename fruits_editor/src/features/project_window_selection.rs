use std::{ffi::{OsStr, OsString}, path::Path};

use crate::{features::generic_dropdown::{GenericDropdownResource, select_generic_dropdown_system}, *};

pub fn register_feature(mut world: WorldBuilderMut) {
    world
        .data_mut()
        .resources_mut()
        .insert(SelectedFileResource::default());
    world
        .data_mut()
        .resources_mut()
        .insert(ProjectEntryContextMenuResource::default());

    let mut behavior = world.behavior_mut();
    let mut update = behavior.get_mut(Schedule::Update);

    update.group(SYSTEM_GROUP)
        .insert_child_system(select_file_system)
        .insert_child_system(update_project_entry_selection_system)
        .insert_child_system(open_project_entry_context_menu_system)
        .insert_child_system(apply_project_entry_context_menu_system);

    update
        .order_system(check_ui_interaction_system)
        .before_system(select_file_system)
        .before_system(update_project_entry_selection_system);

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
    let Some((entry_c, button_c)) = entry_q.iter().filter(|(_, b)| b.was_clicked_this_frame).next() else {
        return;
    };

    let potential_key = get_asset_key(&entry_c.path, &open_project).unwrap_or_else(|| entry_c.path.to_str().unwrap_or("").to_string());

    selected_file_res.path = entry_c.path.clone();
    selected_file_res.potential_asset_key = potential_key;
    select_evt.push(FileSelectedEvent);

    if button_c.was_double_clicked_this_frame {
        open_evt.push(FileOpenedEvent);
    }
}

fn get_asset_key(path: &OsStr, open_project: &OpenProjectResource) -> Option<String> {
    let assets_dir_path = open_project.dir_path.to_string() + PROJECT_ASSETS_SUBPATH;
    let key = path.to_str()?.strip_prefix(&assets_dir_path)?;
    Some(key.replace("\\", "/"))
}

#[derive(Clone, Copy)]
enum ProjectEntryContextAction {
    ShowInExplorer,
    CopyAssetKey,
}

impl ProjectEntryContextAction {
    fn label(self) -> &'static str {
        match self {
            Self::ShowInExplorer => "Show in explorer",
            Self::CopyAssetKey => "Copy asset key",
        }
    }
}

#[derive(Resource, Default)]
pub struct ProjectEntryContextMenuResource {
    request_id: u64,
    path: OsString,
    actions: Vec<ProjectEntryContextAction>,
}

pub fn open_project_entry_context_menu_system(
    input_res: Res<InputResource>,
    entry_q: WorldQuery<(&ProjectWindowEntryComponent, &ButtonComponent)>,
    open_project: Res<OpenProjectResource>,
    mut generic_dropdown_res: ResMut<GenericDropdownResource>,
    mut context_menu_res: ResMut<ProjectEntryContextMenuResource>,
) {
    let Some((entry_c, _)) = entry_q.iter().find(|(_, b)| b.was_right_clicked_this_frame) else {
        return;
    };

    let mut actions = vec![ProjectEntryContextAction::ShowInExplorer];
    if get_asset_key(&entry_c.path, &open_project).is_some() {
        actions.push(ProjectEntryContextAction::CopyAssetKey);
    }

    let position = Vec2::from_array(input_res.mouse.position.map(|v| v as f32));
    context_menu_res.request_id = generic_dropdown_res.request(actions.iter().map(|a| FfiString::from(a.label())).collect(), position);
    context_menu_res.path = entry_c.path.clone();
    context_menu_res.actions = actions;
}

pub fn apply_project_entry_context_menu_system(
    generic_dropdown_res: Res<GenericDropdownResource>,
    open_project: Res<OpenProjectResource>,
    mut context_menu_res: ResMut<ProjectEntryContextMenuResource>,
) {
    let context_menu = &mut *context_menu_res;

    let Some(idx) = generic_dropdown_res.take_selected(&mut context_menu.request_id) else {
        return;
    };

    match context_menu.actions[idx as usize] {
        ProjectEntryContextAction::ShowInExplorer => reveal_in_file_explorer(Path::new(&context_menu.path)),
        ProjectEntryContextAction::CopyAssetKey => {
            if let Some(key) = get_asset_key(&context_menu.path, &open_project) {
                copy_to_clipboard(&key);
            }
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
