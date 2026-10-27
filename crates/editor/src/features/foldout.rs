use std::{collections::BTreeSet, path::PathBuf};

use aphyn::*;

use crate::{
    SYSTEM_GROUP,
    features::{
        inspector_window::{
            data::OpenProjectResource,
            systems::{update_hierarchy_window_system, update_inspector_window_system},
        },
        ui_interaction::check_ui_interaction_system,
        windows_layout::APP_NAME,
    },
    *,
};

const FILE_NAME: &str = "foldouts.json";

pub const FOLDOUT_WIDTH: f32 = 20.0;
const FOLDOUT_ARROW_SIZE: f32 = 16.0;

pub fn register_feature(mut world: WorldBuilderMut) {
    world.data_mut().resources_mut().insert(FoldoutResource::default());

    let mut behavior = world.behavior_mut();

    {
        let mut start = behavior.get_mut(Schedule::Start);

        start.insert_system(load_foldouts_system);
    }

    {
        let mut update = behavior.get_mut(Schedule::Update);

        update.group(SYSTEM_GROUP)
            .insert_child_system(toggle_foldout_system)
            .insert_child_system(update_foldout_arrows_system);

        update.order_system(check_ui_interaction_system).before_system(toggle_foldout_system);
        update.order_system(toggle_foldout_system).before_system(update_foldout_arrows_system);
        update.order_system(toggle_foldout_system).before_system(update_hierarchy_window_system);
        update.order_system(toggle_foldout_system).before_system(update_inspector_window_system);
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum FoldoutKey {
    ProjectEntry { path: String },
    HierarchyEntity { asset_key: String, entity_id: u64 },
    InspectorComponent { component_id: String },
}

#[derive(Component, Debug, Clone)]
pub struct FoldoutButtonComponent {
    pub key: FoldoutKey,
    pub arrow_text: EntityId,
}

#[derive(Event)]
pub struct FoldoutToggledEvent;

#[derive(Serializable, Default, PartialEq, Clone, Debug)]
struct HierarchyEntityFoldout {
    asset_key: String,
    entity_id: u64,
}

#[derive(Serializable, Default, PartialEq, Clone, Debug)]
struct ProjectFoldouts {
    project_dir: String,
    project_entries: Vec<String>,
    hierarchy_entities: Vec<HierarchyEntityFoldout>,
    inspector_components: Vec<String>,
}

#[derive(Serializable, Default, PartialEq, Clone, Debug)]
struct FoldoutsFile {
    projects: Vec<ProjectFoldouts>,
}

#[derive(Resource, Default)]
pub struct FoldoutResource {
    project_dir: String,
    expanded: BTreeSet<FoldoutKey>,
    other_projects: Vec<ProjectFoldouts>,
}

impl FoldoutResource {
    pub fn is_expanded(&self, key: &FoldoutKey) -> bool {
        self.expanded.contains(key)
    }

    fn toggle(&mut self, key: &FoldoutKey) {
        if !self.expanded.remove(key) {
            self.expanded.insert(key.clone());
        }
    }

    fn from_file(project_dir: String, file: FoldoutsFile) -> Self {
        let (current, other_projects): (Vec<_>, Vec<_>) = file.projects.into_iter().partition(|p| p.project_dir == project_dir);

        let mut expanded = BTreeSet::new();

        for project in current {
            expanded.extend(project.project_entries.into_iter().map(|path| FoldoutKey::ProjectEntry { path }));
            expanded.extend(project.hierarchy_entities.into_iter().map(|e| FoldoutKey::HierarchyEntity { asset_key: e.asset_key, entity_id: e.entity_id }));
            expanded.extend(project.inspector_components.into_iter().map(|component_id| FoldoutKey::InspectorComponent { component_id }));
        }

        Self { project_dir, expanded, other_projects }
    }

    fn to_file(&self) -> FoldoutsFile {
        let mut current = ProjectFoldouts { project_dir: self.project_dir.clone(), ..Default::default() };

        for key in &self.expanded {
            match key {
                FoldoutKey::ProjectEntry { path } => current.project_entries.push(path.clone()),
                FoldoutKey::HierarchyEntity { asset_key, entity_id } => current.hierarchy_entities.push(HierarchyEntityFoldout {
                    asset_key: asset_key.clone(),
                    entity_id: *entity_id,
                }),
                FoldoutKey::InspectorComponent { component_id } => current.inspector_components.push(component_id.clone()),
            }
        }

        let mut projects = self.other_projects.clone();
        projects.push(current);

        FoldoutsFile { projects }
    }
}

fn foldouts_file_path() -> PathBuf {
    save_path(APP_NAME).join(FILE_NAME)
}

fn load_foldouts_file() -> Option<FoldoutsFile> {
    let path = foldouts_file_path();

    let json_str = std::fs::read_to_string(&path).ok()?;

    let json = match serde_json::from_str(&json_str) {
        Ok(json) => json,
        Err(err) => {
            eprintln!("failed to parse {:?}. {}", &path, err);
            return None;
        }
    };

    let mut errors = Vec::new();
    let mut on_err = |err: SerializationError| errors.push(err.to_string());
    let mut ctx = SerializerCtx::new(PureSerializerCtxState, &mut on_err);

    let file: FoldoutsFile = ctx.deserialize_default("", &SerializedValue::from_json(&json));

    if !errors.is_empty() {
        eprintln!("failed to deserialize {:?}. {}", &path, errors.join("; "));
        return None;
    }

    Some(file)
}

fn store_foldouts_file(file: &FoldoutsFile) {
    let mut on_err = |err: SerializationError| eprintln!("failed to serialize foldouts. {err}");
    let mut ctx = SerializerCtx::new(PureSerializerCtxState, &mut on_err);

    let json_str = serde_json::to_string_pretty(&ctx.serialize(file, "").to_json()).unwrap();

    let path = foldouts_file_path();

    if let Some(dir) = path.parent()
        && let Err(err) = std::fs::create_dir_all(dir)
    {
        eprintln!("failed to create {:?}. {}", dir, err);
        return;
    }

    if let Err(err) = std::fs::write(&path, json_str.into_bytes()) {
        eprintln!("failed to write to {:?}. {}", &path, err);
    }
}

fn load_foldouts_system(open_project: Res<OpenProjectResource>, mut foldout_res: ResMut<FoldoutResource>) {
    let project_dir = std::fs::canonicalize(&open_project.dir_path)
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|_| open_project.dir_path.clone());

    *foldout_res = FoldoutResource::from_file(project_dir, load_foldouts_file().unwrap_or_default());
}

pub fn toggle_foldout_system(
    button_q: WorldQuery<(&FoldoutButtonComponent, &ButtonComponent)>,
    mut project_window_q: WorldQuery<&mut ProjectWindowComponent>,
    mut foldout_res: ResMut<FoldoutResource>,
    mut toggled_evt: EvtMut<FoldoutToggledEvent>,
) {
    let toggled = button_q.iter().filter(|(_, b)| b.was_clicked_this_frame).map(|(f, _)| f.key.clone()).collect::<Vec<_>>();

    return_if!(toggled.is_empty());

    for key in &toggled {
        foldout_res.toggle(key);
    }

    if toggled.iter().any(|k| matches!(k, FoldoutKey::ProjectEntry { .. })) {
        for project_win_c in project_window_q.iter_mut() {
            project_win_c.is_init = false;
        }
    }

    store_foldouts_file(&foldout_res.to_file());
    toggled_evt.push(FoldoutToggledEvent);
}

fn update_foldout_arrows_system(
    foldout_res: Res<FoldoutResource>,
    button_q: WorldQuery<&FoldoutButtonComponent>,
    mut text_q: WorldQuery<&mut TextComponent>,
) {
    for foldout_c in button_q.iter() {
        continue_if_not!(Some(text_c) = text_q.get_mut(foldout_c.arrow_text));

        let arrow = arrow_str(foldout_res.is_expanded(&foldout_c.key));

        if text_c.text.as_str() != arrow {
            text_c.text = arrow.into();
        }
    }
}

fn arrow_str(is_expanded: bool) -> &'static str {
    if is_expanded { "v" } else { ">" }
}

// the name is padded past the arrow column even when there is no arrow, so siblings line up
pub fn spawn_foldout_arrow(
    mut ec: EntitiesHolderMut,
    ent_row: EntityId,
    ent_name: EntityId,
    indent: f32,
    foldout: Option<(FoldoutKey, bool)>,
    material_panel: &AssetHandle<StandardMaterial>,
    material_text: &AssetHandle<StandardMaterial>,
    font: &AssetHandle<Font>,
) -> EntityId {
    if let Some(local_rect_c) = ec.get_component_mut::<LocalRectComponent>(ent_name) {
        local_rect_c.parent_padding_min.x = UiVal::px(indent + FOLDOUT_WIDTH);
    }

    let Some((key, is_expanded)) = foldout else {
        return EntityId::EMPTY;
    };

    let ent_arrow = ec.create_entity();
    let ent_arrow_text = ec.create_entity();

    ec.set_components(ent_arrow, (
        GlobalRectComponent::default(),
        LocalRectComponent {
            anchor: Vec2::new(0.0, 0.5),
            pivot: Vec2::new(0.0, 0.5),
            offset: Vec2::new(UiVal::px(indent + (FOLDOUT_WIDTH - FOLDOUT_ARROW_SIZE) * 0.5), UiVal::px(0.0)),
            scale: Vec2::splat(Some(UiVal::px(FOLDOUT_ARROW_SIZE)).into()),
            ..Default::default()
        },
        ChildComponent { parent: ent_row },
        ParentComponent { children: vec![ent_arrow_text].into() },
        ButtonComponent::default(),
        FoldoutButtonComponent { key, arrow_text: ent_arrow_text },
        BatchedMeshComponent::default(),
        StandardMaterialComponent { material: material_panel.clone() },
        ImageComponent {
            color: Vec4::from_array(rgba_f32_array!("#00000030")),
            ..Default::default()
        },
    ));
    if let Some(parent_c) = ec.get_component_mut::<ParentComponent>(ent_row) {
        parent_c.children.push(ent_arrow);
    }

    ec.set_components(ent_arrow_text, (
        GlobalRectComponent::default(),
        LocalRectComponent::default(),
        ChildComponent { parent: ent_arrow },
        BatchedMeshComponent::default(),
        StandardMaterialComponent { material: material_text.clone() },
        TextComponent {
            color: Vec4::from_array(parse_color_rgba_f32("#000000ff").unwrap()),
            font: font.clone(),
            font_size: UiVal::px(16.0),
            is_y_inverted: true,
            text: arrow_str(is_expanded).into(),
            horizontal_spacing: UiVal::px(0.0),
            vertical_align: VerticalAlign::Middle,
            horizontal_align: HorizontalAlign::Middle,
        },
    ));

    ent_arrow
}
