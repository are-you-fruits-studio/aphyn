use std::ffi::OsString;

use crate::*;

pub fn register_feature(mut world: WorldBuilderMut) {
    world
        .data_mut()
        .resources_mut()
        .insert(SelectedFileResource::default());

    let mut behavior = world.behavior_mut();
    let mut update = behavior.get_mut(Schedule::Update);

    update.group(SYSTEM_GROUP)
        .insert_child_system(select_file_system)
        .insert_child_system(update_project_entry_selection_system);

    update
        .order_system(check_ui_interaction_system)
        .before_system(select_file_system)
        .before_system(update_project_entry_selection_system);
}

#[derive(Resource, Clone, Default)]
pub struct SelectedFileResource {
    pub path: OsString,
    pub potential_asset_key: String,
}

#[derive(Event, Copy, Clone, Default)]
pub struct FileSelectedEvent;

pub fn select_file_system(
    entry_q: WorldQuery<(&ProjectWindowEntryComponent, &ButtonComponent)>,
    open_project: Res<OpenProjectResource>,
    mut selected_file_res: ResMut<SelectedFileResource>,
    mut select_evt: EvtMut<FileSelectedEvent>,
) {
    let Some(entry_c) = entry_q.iter().filter(|(e, b)| b.was_clicked_this_frame).map(|(e, b)| e).next() else {
        return;
    };

    let mut potential_key = entry_c.path.to_str().unwrap_or("").to_string();

    let assets_dir_path = open_project.dir_path.to_string() + PROJECT_ASSETS_SUBPATH;

    if potential_key.starts_with(&assets_dir_path) {
        potential_key = potential_key.chars().skip(assets_dir_path.chars().count()).collect::<String>();
        potential_key = potential_key.replace("\\", "/")
    }

    selected_file_res.path = entry_c.path.clone();
    selected_file_res.potential_asset_key = potential_key;
    select_evt.push(FileSelectedEvent);
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
