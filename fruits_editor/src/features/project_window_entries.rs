use std::path::PathBuf;

use crate::{features::inspector_window::utils::entries::spawn_hierarchy_entry, prefabs::WindowComponent, *};

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

#[derive(Resource, Debug, Default)]
pub struct ProjectWindowCache {
    pub dir_entry: ProjectWindowDataEntry,
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

    let current_dir = PathBuf::from(open_project.dir_path + PROJECT_ASSETS_SUBPATH);

    let cache = res.get_mut::<ProjectWindowCache>().unwrap();

    let entry = ProjectWindowDataEntry::scan(&current_dir);

    let is_cache_old = cache.dir_entry != entry;

    if is_cache_old {
        cache.dir_entry = entry.clone();
    }

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
                content,
                entry,
            );
        }
    }
}

fn spawn_project_window_entries(
    mut ec: EntitiesHolderMut,
    material_text: &AssetHandle<StandardMaterial>,
    material_panel: &AssetHandle<StandardMaterial>,
    font: &AssetHandle<Font>,
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

    for entry in &entry.children {
        spawn_project_window_entries(
            ec.as_mut(),
            material_text,
            material_panel,
            font,
            spawned.ent_children_container,
            entry,
        );
    }
}
