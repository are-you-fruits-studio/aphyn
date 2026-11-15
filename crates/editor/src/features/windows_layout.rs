use std::path::PathBuf;

use aphyn::*;

use crate::{
    SYSTEM_GROUP,
    features::{
        docking::{DockLayoutResource, DockNode, spawn_splitter},
        test_window::{open_panel_by_kind, open_windows_system},
        ui_window::{
            WindowsOrderResource, attach_panel, order_windows_system, px_offset, px_scale, set_px_rect, sync_window_groups_system,
        },
    },
    prefabs::{WindowComponent, WindowGroupComponent, window_group},
};

pub const APP_NAME: &str = "aphyn_editor";
const FILE_NAME: &str = "windows_layout.json";

// todo: bundle registration params
pub fn register_feature(_data: WorldDataMut, mut behavior: WorldBehaviorBuilderMut, _render: &mut RenderFeatureRegistry) {
    {
        let mut start = behavior.get_mut(Schedule::Start);

        start.insert_system(restore_windows_layout_system);

        start
            .order_system(crate::init_system)
            .before_system(restore_windows_layout_system);
    }

    {
        let mut update = behavior.get_mut(Schedule::Update);

        update.group(SYSTEM_GROUP).insert_child_system(save_windows_layout_system);

        update.order_system(open_windows_system).before_system(order_windows_system);
        update
            .order_system(open_windows_system)
            .before_system(save_windows_layout_system);
        update
            .order_system(sync_window_groups_system)
            .before_system(save_windows_layout_system);
    }
}

// the dock tree is stored in pre-order: a split is followed by its two subtrees
#[derive(Serializable, Default, PartialEq, Clone, Debug)]
struct DockNodeLayout {
    is_split: bool,
    tabs: Vec<String>,
    active: u32,
    axis: u32,
    ratio: f32,
}

#[derive(Serializable, Default, PartialEq, Clone, Debug)]
struct FloatingGroupLayout {
    tabs: Vec<String>,
    active: u32,
    offset_x: f32,
    offset_y: f32,
    width: f32,
    height: f32,
    order: u32,
}

#[derive(Serializable, Default, PartialEq, Clone, Debug)]
struct WindowsLayout {
    dock: Vec<DockNodeLayout>,
    floating: Vec<FloatingGroupLayout>,
}

#[derive(Resource, Default)]
pub struct WindowsLayoutResource {
    last_saved: WindowsLayout,
}

fn layout_file_path() -> PathBuf {
    save_path(APP_NAME).join(FILE_NAME)
}

fn load_layout() -> Option<WindowsLayout> {
    let path = layout_file_path();

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

    let layout: WindowsLayout = ctx.deserialize_default("", &SerializedValue::from_json(&json));

    if !errors.is_empty() {
        eprintln!("failed to deserialize {:?}. {}", &path, errors.join("; "));
        return None;
    }

    Some(layout)
}

fn store_layout(layout: &WindowsLayout) {
    let mut on_err = |err: SerializationError| eprintln!("failed to serialize windows layout. {err}");
    let mut ctx = SerializerCtx::new(PureSerializerCtxState, &mut on_err);

    let json_str = serde_json::to_string_pretty(&ctx.serialize(layout, "").to_json()).unwrap();

    let path = layout_file_path();

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

fn open_group_with_tabs(mut world: WorldDataMut, tabs: &[String], active: u32) -> Option<EntityId> {
    let panels = tabs
        .iter()
        .filter_map(|kind| open_panel_by_kind(world.as_mut(), kind))
        .collect::<Vec<_>>();

    if panels.is_empty() {
        return None;
    }

    let ent_group = window_group(world.as_mut());

    for &ent_panel in &panels {
        attach_panel(world.entities_mut(), ent_group, ent_panel);
    }

    let mut ec = world.entities_mut();
    let group_c = ec.get_component_mut::<WindowGroupComponent>(ent_group).unwrap();
    group_c.active = (active as u64).min(panels.len() as u64 - 1);

    Some(ent_group)
}

fn restore_dock_node(mut world: WorldDataMut, nodes: &mut std::slice::Iter<DockNodeLayout>) -> Option<DockNode<EntityId>> {
    let node = nodes.next()?;

    if node.is_split {
        let first = restore_dock_node(world.as_mut(), nodes);
        let second = restore_dock_node(world.as_mut(), nodes);

        return match (first, second) {
            (Some(first), Some(second)) => Some(DockNode::Split {
                axis: (node.axis as usize).min(1),
                ratio: node.ratio,
                splitter: spawn_splitter(world.as_mut()),
                children: Box::new([first, second]),
            }),
            (Some(single), None) | (None, Some(single)) => Some(single),
            (None, None) => None,
        };
    }

    let ent_group = open_group_with_tabs(world.as_mut(), &node.tabs, node.active)?;
    world
        .entities_mut()
        .get_component_mut::<WindowGroupComponent>(ent_group)
        .unwrap()
        .is_docked = true;

    Some(DockNode::Leaf(ent_group))
}

fn restore_windows_layout_system(mut world: WorldDataMut) {
    let layout = load_layout().unwrap_or_default();

    let dock_root = restore_dock_node(world.as_mut(), &mut layout.dock.iter());
    world.resources_mut().get_mut::<DockLayoutResource>().unwrap().tree.root = dock_root;

    let mut ordered_groups = Vec::new();

    for entry in &layout.floating {
        continue_if_not!(Some(ent_group) = open_group_with_tabs(world.as_mut(), &entry.tabs, entry.active));

        ordered_groups.push((entry.order, ent_group));

        if let Some(rect_c) = world.entities_mut().get_component_mut::<LocalRectComponent>(ent_group) {
            set_px_rect(
                rect_c,
                Vec2::new(entry.offset_x, entry.offset_y),
                Vec2::new(entry.width, entry.height),
            );
        }
    }

    ordered_groups.sort_by_key(|&(order, _)| order);

    world.resources_mut().get_mut::<WindowsOrderResource>().unwrap().front_to_back =
        ordered_groups.into_iter().map(|(_, ent)| ent).collect();

    world.resources_mut().insert(WindowsLayoutResource { last_saved: layout });
}

fn group_tabs(group_c: &WindowGroupComponent, window_q: &WorldQuery<&WindowComponent>) -> Vec<String> {
    group_c
        .panels
        .iter()
        .filter_map(|&ent_panel| window_q.get(ent_panel).map(|window_c| window_c.kind.to_string()))
        .collect()
}

fn save_dock_node(
    node: &DockNode<EntityId>,
    group_q: &WorldQuery<(EntityId, &LocalRectComponent, &WindowGroupComponent)>,
    window_q: &WorldQuery<&WindowComponent>,
    out: &mut Vec<DockNodeLayout>,
) {
    match node {
        DockNode::Leaf(ent_group) => {
            let group_c = group_q.get(*ent_group).map(|(_, _, group_c)| group_c);

            out.push(DockNodeLayout {
                tabs: group_c.map(|group_c| group_tabs(group_c, window_q)).unwrap_or_default(),
                active: group_c.map(|group_c| group_c.active as u32).unwrap_or(0),
                ..Default::default()
            });
        }
        DockNode::Split {
            axis, ratio, children, ..
        } => {
            out.push(DockNodeLayout {
                is_split: true,
                axis: *axis as u32,
                ratio: *ratio,
                ..Default::default()
            });

            for child in children.iter() {
                save_dock_node(child, group_q, window_q, out);
            }
        }
    }
}

pub fn save_windows_layout_system(
    input_res: Res<InputResource>,
    order_res: Res<WindowsOrderResource>,
    dock_res: Res<DockLayoutResource>,
    mut layout_res: ResMut<WindowsLayoutResource>,
    group_q: WorldQuery<(EntityId, &LocalRectComponent, &WindowGroupComponent)>,
    window_q: WorldQuery<&WindowComponent>,
) {
    if input_res.mouse.is_pressed(MouseButton::Left) {
        return;
    }

    let mut dock = Vec::new();

    if let Some(root) = &dock_res.tree.root {
        save_dock_node(root, &group_q, &window_q, &mut dock);
    }

    let mut floating = group_q
        .iter()
        .filter(|(_, _, group_c)| !group_c.is_docked)
        .map(|(ent_group, rect_c, group_c)| {
            let order = order_res
                .front_to_back
                .iter()
                .position(|&e| e == ent_group)
                .unwrap_or(order_res.front_to_back.len());

            FloatingGroupLayout {
                tabs: group_tabs(group_c, &window_q),
                active: group_c.active as u32,
                offset_x: px_offset(rect_c, 0),
                offset_y: px_offset(rect_c, 1),
                width: px_scale(rect_c, 0),
                height: px_scale(rect_c, 1),
                order: order as u32,
            }
        })
        .collect::<Vec<_>>();

    floating.sort_by_key(|entry| entry.order);

    let layout = WindowsLayout { dock, floating };

    if layout == layout_res.last_saved {
        return;
    }

    store_layout(&layout);

    layout_res.last_saved = layout;
}
