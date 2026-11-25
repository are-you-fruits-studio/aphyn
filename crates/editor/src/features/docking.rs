use aphyn::*;

use crate::{
    SYSTEM_GROUP,
    features::{
        ui_interaction::{DragAreaComponent, HoverAreaComponent},
        ui_window::{
            Bounds, DockComponent, MIN_WINDOW_SIZE, WindowsOrderResource, attach_panel, clamp_floating_rect, detach_panel,
            dock_bounds_of, px_offset, px_scale, rescale_window_system, set_active_panel, set_px_rect, window_tabs_system,
        },
    },
    prefabs::{WINDOW_HEADER_HEIGHT, WindowComponent, WindowGroupComponent, window_group},
    resources::StandardAssetsResource,
};

const DOCK_GAP: f32 = 4.0;
const UNDOCK_THRESHOLD: f32 = 20.0;
const DOCK_EDGE_BAND: f32 = 24.0;
const GROUP_EDGE_FRACTION: f32 = 0.25;
const GROUP_EDGE_MAX: f32 = 60.0;
const CENTER_ZONE_FRACTION: f32 = 0.3;
const CENTER_ZONE_MAX: f32 = 80.0;
const ROOT_SPLIT_RATIO: f32 = 0.3;
const LEAF_SPLIT_RATIO: f32 = 0.5;
const HIGHLIGHT_THICKNESS: f32 = 3.0;
const HIGHLIGHT_Z: f32 = -8_500.0;

const SPLITTER_COLOR: Vec4<f32> = Vec4::new(0.1, 0.1, 0.1, 1.0);
const SPLITTER_COLOR_HOVERED: Vec4<f32> = Vec4::new(0.38, 0.71, 0.98, 1.0);

pub fn register_feature(ctx: FeatureInitCtx) {
    let FeatureInitCtx {
        mut data, mut behavior, ..
    } = ctx;
    data.resources_mut().insert(DockLayoutResource::default());
    data.resources_mut().insert(DockDragResource::default());

    let mut update = behavior.get_mut(Schedule::Update);

    update
        .group(SYSTEM_GROUP)
        .insert_child_system(dock_drag_system)
        .insert_child_system(drag_splitter_system)
        .insert_child_system(dock_layout_system);

    update.order_system(window_tabs_system).before_system(dock_drag_system);
    update.order_system(dock_drag_system).before_system(drag_splitter_system);
    update.order_system(drag_splitter_system).before_system(dock_layout_system);
    update.order_system(dock_layout_system).before_system(rescale_window_system);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DockSide {
    Left,
    Right,
    Top,
    Bottom,
}

impl DockSide {
    pub fn axis(self) -> usize {
        match self {
            DockSide::Left | DockSide::Right => 0,
            DockSide::Top | DockSide::Bottom => 1,
        }
    }

    pub fn is_min(self) -> bool {
        matches!(self, DockSide::Left | DockSide::Top)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum DockNode<T> {
    Leaf(T),
    Split {
        axis: usize,
        ratio: f32,
        splitter: T,
        children: Box<[DockNode<T>; 2]>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DockLayoutItem<T> {
    Leaf {
        leaf: T,
        min: Vec2<f32>,
        max: Vec2<f32>,
    },
    Splitter {
        splitter: T,
        axis: usize,
        min: Vec2<f32>,
        max: Vec2<f32>,
        parent_min: Vec2<f32>,
        parent_max: Vec2<f32>,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct DockTree<T> {
    pub root: Option<DockNode<T>>,
}

impl<T> Default for DockTree<T> {
    fn default() -> Self {
        Self { root: None }
    }
}

impl<T: Copy + PartialEq> DockTree<T> {
    pub fn insert_at_root(&mut self, leaf: T, side: DockSide, ratio: f32, make_splitter: impl FnOnce() -> T) {
        self.root = Some(match self.root.take() {
            None => DockNode::Leaf(leaf),
            Some(existing) => split_node(existing, leaf, side, ratio, make_splitter()),
        });
    }

    pub fn insert_at_leaf(&mut self, target: T, leaf: T, side: DockSide, ratio: f32, make_splitter: impl FnOnce() -> T) -> bool {
        let Some(node) = self.root.as_mut().and_then(|root| find_leaf_mut(root, target)) else {
            return false;
        };

        let existing = std::mem::replace(node, DockNode::Leaf(leaf));
        *node = split_node(existing, leaf, side, ratio, make_splitter());

        true
    }

    // Some(Some(splitter)) when a split collapsed, Some(None) when the root leaf was removed
    pub fn remove(&mut self, leaf: T) -> Option<Option<T>> {
        let root = self.root.as_mut()?;

        if matches!(root, DockNode::Leaf(l) if *l == leaf) {
            self.root = None;
            return Some(None);
        }

        remove_from_node(root, leaf)
    }

    pub fn set_ratio(&mut self, splitter: T, ratio: f32) -> bool {
        self.root.as_mut().is_some_and(|node| set_node_ratio(node, splitter, ratio))
    }

    pub fn layout(&self, min: Vec2<f32>, max: Vec2<f32>, gap: f32, min_size: Vec2<f32>, f: &mut impl FnMut(DockLayoutItem<T>)) {
        if let Some(root) = &self.root {
            layout_node(root, min, max, gap, min_size, f);
        }
    }
}

fn split_node<T>(existing: DockNode<T>, leaf: T, side: DockSide, ratio: f32, splitter: T) -> DockNode<T> {
    let new = DockNode::Leaf(leaf);

    let (children, ratio) = match side.is_min() {
        true => ([new, existing], ratio),
        false => ([existing, new], 1.0 - ratio),
    };

    DockNode::Split {
        axis: side.axis(),
        ratio,
        splitter,
        children: Box::new(children),
    }
}

fn find_leaf_mut<T: Copy + PartialEq>(node: &mut DockNode<T>, leaf: T) -> Option<&mut DockNode<T>> {
    if matches!(node, DockNode::Leaf(l) if *l == leaf) {
        return Some(node);
    }

    match node {
        DockNode::Leaf(_) => None,
        DockNode::Split { children, .. } => children.iter_mut().find_map(|child| find_leaf_mut(child, leaf)),
    }
}

fn remove_from_node<T: Copy + PartialEq>(node: &mut DockNode<T>, leaf: T) -> Option<Option<T>> {
    let DockNode::Split { splitter, children, .. } = node else {
        return None;
    };

    let splitter = *splitter;

    if let Some(idx) = children
        .iter()
        .position(|child| matches!(child, DockNode::Leaf(l) if *l == leaf))
    {
        let sibling = std::mem::replace(&mut children[1 - idx], DockNode::Leaf(leaf));
        *node = sibling;
        return Some(Some(splitter));
    }

    children.iter_mut().find_map(|child| remove_from_node(child, leaf))
}

fn set_node_ratio<T: Copy + PartialEq>(node: &mut DockNode<T>, target: T, new_ratio: f32) -> bool {
    match node {
        DockNode::Leaf(_) => false,
        DockNode::Split {
            ratio,
            splitter,
            children,
            ..
        } => {
            if *splitter == target {
                *ratio = new_ratio;
                return true;
            }

            children.iter_mut().any(|child| set_node_ratio(child, target, new_ratio))
        }
    }
}

fn layout_node<T: Copy>(
    node: &DockNode<T>,
    min: Vec2<f32>,
    max: Vec2<f32>,
    gap: f32,
    min_size: Vec2<f32>,
    f: &mut impl FnMut(DockLayoutItem<T>),
) {
    match node {
        DockNode::Leaf(leaf) => f(DockLayoutItem::Leaf { leaf: *leaf, min, max }),
        DockNode::Split {
            axis,
            ratio,
            splitter,
            children,
        } => {
            let axis = *axis;
            let avail = (max[axis] - min[axis] - gap).max(0.0);
            let lowest = min_size[axis].min(avail * 0.5);
            let first = (avail * ratio).clamp(lowest, avail - lowest);

            let mut first_max = max;
            first_max[axis] = min[axis] + first;

            let mut second_min = min;
            second_min[axis] = first_max[axis] + gap;

            let mut splitter_min = min;
            splitter_min[axis] = first_max[axis];

            let mut splitter_max = max;
            splitter_max[axis] = second_min[axis];

            f(DockLayoutItem::Splitter {
                splitter: *splitter,
                axis,
                min: splitter_min,
                max: splitter_max,
                parent_min: min,
                parent_max: max,
            });

            layout_node(&children[0], min, first_max, gap, min_size, f);
            layout_node(&children[1], second_min, max, gap, min_size, f);
        }
    }
}

#[derive(Resource, Default)]
pub struct DockLayoutResource {
    pub tree: DockTree<EntityId>,
}

#[derive(Resource, Default)]
pub struct DockDragResource {
    dragging: Option<(EntityId, Vec2<f32>)>,
    highlight: Option<EntityId>,
}

#[derive(Component, Debug, Copy, Clone, Default)]
pub struct DockSplitterComponent {
    pub axis: u32,
    pub parent_min: Vec2<f32>,
    pub parent_max: Vec2<f32>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum DropTarget {
    DockFill,
    DockEdge(DockSide),
    GroupEdge(EntityId, DockSide),
    GroupCenter(EntityId),
}

pub fn remove_group_from_dock(mut world: WorldDataMut, ent_group: EntityId) {
    let (mut res, mut ec, _evt) = world.as_tuple_mut();

    let removed = res.get_mut::<DockLayoutResource>().unwrap().tree.remove(ent_group);

    if let Some(Some(ent_splitter)) = removed {
        destroy_entity_and_children(ec.as_mut(), ent_splitter);
    }

    if let Some(group_c) = ec.get_component_mut::<WindowGroupComponent>(ent_group) {
        group_c.is_docked = false;
    }
}

pub fn spawn_splitter(mut world: WorldDataMut) -> EntityId {
    let (res, mut ec, _evt) = world.as_tuple_mut();

    let material_panel = res.as_ref().get::<StandardAssetsResource>().unwrap().material_panel.clone();

    let ent_splitter = ec.create_entity();

    ec.set_components(
        ent_splitter,
        (
            GlobalRectComponent::default(),
            LocalRectComponent {
                anchor: Vec2::new(0.0, 0.0),
                pivot: Vec2::new(0.0, 0.0),
                scale: Vec2::splat(UiVal::px(0.0).into()),
                ..Default::default()
            },
            BatchedMeshComponent::default(),
            MaterialComponent {
                material: material_panel,
            },
            ImageComponent {
                color: SPLITTER_COLOR,
                ..Default::default()
            },
            HoverAreaComponent::default(),
            DragAreaComponent::default(),
            DockSplitterComponent::default(),
        ),
    );

    ent_splitter
}

fn spawn_highlight(mut world: WorldDataMut) -> EntityId {
    let (res, mut ec, _evt) = world.as_tuple_mut();

    let material_panel = res.as_ref().get::<StandardAssetsResource>().unwrap().material_panel.clone();

    let ent_root = ec.create_entity();
    let edges = [(); 4].map(|_| ec.create_entity());

    ec.set_components(
        ent_root,
        (
            GlobalRectComponent::default(),
            GlobalDisableableComponent::default(),
            LocalDisableableComponent { is_disabled: true },
            LocalRectComponent {
                anchor: Vec2::new(0.0, 0.0),
                pivot: Vec2::new(0.0, 0.0),
                scale: Vec2::splat(UiVal::px(0.0).into()),
                z: HIGHLIGHT_Z,
                ..Default::default()
            },
        ),
    );

    let edge_rects = [
        (Vec2::new(0.0, 0.5), Vec2::new(UiVal::px(HIGHLIGHT_THICKNESS), UiVal::pd(1.0))),
        (Vec2::new(1.0, 0.5), Vec2::new(UiVal::px(HIGHLIGHT_THICKNESS), UiVal::pd(1.0))),
        (Vec2::new(0.5, 0.0), Vec2::new(UiVal::pd(1.0), UiVal::px(HIGHLIGHT_THICKNESS))),
        (Vec2::new(0.5, 1.0), Vec2::new(UiVal::pd(1.0), UiVal::px(HIGHLIGHT_THICKNESS))),
    ];

    for (ent_edge, (anchor, scale)) in edges.into_iter().zip(edge_rects) {
        ec.set_components(
            ent_edge,
            (
                GlobalRectComponent::default(),
                GlobalDisableableComponent::default(),
                LocalRectComponent {
                    anchor,
                    pivot: anchor,
                    scale: scale.map(|v| v.into()),
                    ..Default::default()
                },
                BatchedMeshComponent::default(),
                MaterialComponent {
                    material: material_panel.clone(),
                },
                ImageComponent {
                    color: SPLITTER_COLOR_HOVERED,
                    ..Default::default()
                },
            ),
        );
        set_parent(ec.as_mut(), ent_edge, ent_root);
    }

    ent_root
}

fn mouse_position(world: &mut WorldDataMut) -> (Vec2<f32>, bool) {
    let res = world.resources_mut();
    let input = res.get::<InputResource>().unwrap();

    (
        Vec2::from_array(input.mouse.position.map(|v| v as f32)),
        input.mouse.is_pressed(MouseButton::Left),
    )
}

fn current_dock_bounds(ec: EntitiesHolderRef) -> Option<Bounds> {
    ec.query_filtered::<&GlobalRectComponent, WithFilter<DockComponent>>()
        .iter()
        .next()
        .and_then(dock_bounds_of)
}

fn group_bounds(ec: EntitiesHolderRef, ent_group: EntityId) -> Option<Bounds> {
    ec.get_component::<GlobalRectComponent>(ent_group).map(Bounds::of_rect)
}

pub fn dock_drag_system(mut world: WorldDataMut) {
    let (mouse_pos, is_left_pressed) = mouse_position(&mut world);

    let (highlight, dragging) = {
        let res = world.resources_mut();
        let drag_res = res.get::<DockDragResource>().unwrap();
        (drag_res.highlight, drag_res.dragging)
    };

    let ent_highlight = match highlight {
        Some(ent) => ent,
        None => {
            let ent = spawn_highlight(world.as_mut());
            world.resources_mut().get_mut::<DockDragResource>().unwrap().highlight = Some(ent);
            ent
        }
    };

    let bounds = current_dock_bounds(world.entities());
    let tree = world.resources_mut().get::<DockLayoutResource>().unwrap().tree.clone();

    let dragging = dragging.filter(|(ent_group, _)| world.entities().get_component::<WindowGroupComponent>(*ent_group).is_some());

    let highlight = match dragging {
        None => {
            let started = try_start_drag(world.as_mut(), mouse_pos, bounds);
            world.resources_mut().get_mut::<DockDragResource>().unwrap().dragging = started;
            None
        }
        Some((ent_group, _)) if !is_left_pressed => {
            let target = bounds.and_then(|bounds| find_drop_target(world.entities(), &tree, ent_group, mouse_pos, bounds));

            if let Some((target, _)) = target {
                apply_drop(world.as_mut(), ent_group, target);
            }

            world.resources_mut().get_mut::<DockDragResource>().unwrap().dragging = None;
            None
        }
        Some((ent_group, grab)) => {
            if let Some(rect_c) = world.entities_mut().get_component_mut::<LocalRectComponent>(ent_group) {
                let size = Vec2::from_fn(|axis| px_scale(rect_c, axis));
                let offset = match bounds {
                    Some(bounds) => clamp_floating_rect(mouse_pos - grab, size, bounds).0,
                    None => mouse_pos - grab,
                };
                rect_c.offset = offset.map(UiVal::px);
            }

            bounds.and_then(|bounds| find_drop_target(world.entities(), &tree, ent_group, mouse_pos, bounds))
        }
    };

    let mut ec = world.entities_mut();

    if let Some(disableable_c) = ec.get_component_mut::<LocalDisableableComponent>(ent_highlight) {
        disableable_c.is_disabled = highlight.is_none();
    }

    if let Some((_, rect)) = highlight
        && let Some(rect_c) = ec.get_component_mut::<LocalRectComponent>(ent_highlight)
    {
        set_px_rect(rect_c, rect.min, rect.size());
    }
}

fn try_start_drag(mut world: WorldDataMut, mouse_pos: Vec2<f32>, bounds: Option<Bounds>) -> Option<(EntityId, Vec2<f32>)> {
    let mut pulled_tab = None;
    let mut pressed_header = None;

    {
        let ec = world.entities();

        for (ent_group, group_c) in ec.query::<(EntityId, &WindowGroupComponent)>().iter() {
            for &ent_panel in group_c.panels.iter() {
                continue_if_not!(Some(window_c) = ec.get_component::<WindowComponent>(ent_panel));
                continue_if_not!(Some(drag_c) = ec.get_component::<DragAreaComponent>(window_c.tab));
                continue_if_not!(Some(press_offset) = drag_c.press_global_offset);
                continue_if_not!(Some(tab_rect) = ec.get_component::<GlobalRectComponent>(window_c.tab));

                let press_pos = tab_rect.center + press_offset;
                let delta = mouse_pos - press_pos;

                if (delta.x * delta.x + delta.y * delta.y).sqrt() > UNDOCK_THRESHOLD {
                    pulled_tab = Some((ent_group, ent_panel, window_c.tab, press_pos));
                }
            }

            if !group_c.is_docked
                && ec
                    .get_component::<DragAreaComponent>(group_c.header)
                    .is_some_and(|drag_c| drag_c.press_global_offset.is_some())
            {
                pressed_header = Some(ent_group);
            }
        }
    }

    if let Some((ent_group, ent_panel, ent_tab, press_pos)) = pulled_tab {
        if let Some(drag_c) = world.entities_mut().get_component_mut::<DragAreaComponent>(ent_tab) {
            drag_c.press_global_offset = None;
        }

        return pull_panel_out(world, ent_group, ent_panel, press_pos, mouse_pos, bounds);
    }

    let ent_group = pressed_header?;
    let rect_c = world.entities().get_component::<LocalRectComponent>(ent_group)?;
    let offset = Vec2::from_fn(|axis| px_offset(rect_c, axis));

    Some((ent_group, mouse_pos - offset))
}

fn pull_panel_out(
    mut world: WorldDataMut,
    ent_group: EntityId,
    ent_panel: EntityId,
    press_pos: Vec2<f32>,
    mouse_pos: Vec2<f32>,
    bounds: Option<Bounds>,
) -> Option<(EntityId, Vec2<f32>)> {
    let group_c = world.entities().get_component::<WindowGroupComponent>(ent_group)?.clone();
    let group_rect = group_bounds(world.entities(), ent_group)?;

    if group_c.panels.len() == 1 && !group_c.is_docked {
        let rect_c = world.entities().get_component::<LocalRectComponent>(ent_group)?;
        let offset = Vec2::from_fn(|axis| px_offset(rect_c, axis));

        return Some((ent_group, mouse_pos - offset));
    }

    let ent_floating = if group_c.panels.len() == 1 {
        remove_group_from_dock(world.as_mut(), ent_group);
        ent_group
    } else {
        detach_panel(world.entities_mut(), ent_group, ent_panel);
        let ent_new_group = window_group(world.as_mut());
        attach_panel(world.entities_mut(), ent_new_group, ent_panel);
        ent_new_group
    };

    let size = match bounds {
        Some(bounds) => Vec2::from_fn(|axis| group_rect.size()[axis].min(bounds.size()[axis]).max(MIN_WINDOW_SIZE[axis])),
        None => group_rect.size(),
    };
    let grab = Vec2::new(
        (press_pos.x - group_rect.min.x).clamp(0.0, size.x),
        WINDOW_HEADER_HEIGHT * 0.5,
    );

    if let Some(rect_c) = world.entities_mut().get_component_mut::<LocalRectComponent>(ent_floating) {
        set_px_rect(rect_c, mouse_pos - grab, size);
    }

    Some((ent_floating, grab))
}

fn side_bounds(bounds: Bounds, side: DockSide, fraction: f32) -> Bounds {
    let axis = side.axis();
    let extent = bounds.size()[axis] * fraction;
    let mut result = bounds;

    match side.is_min() {
        true => result.max[axis] = bounds.min[axis] + extent,
        false => result.min[axis] = bounds.max[axis] - extent,
    }

    result
}

fn nearest_side(bounds: Bounds, pos: Vec2<f32>, band: Vec2<f32>) -> Option<DockSide> {
    [
        (DockSide::Left, pos.x - bounds.min.x, band.x),
        (DockSide::Right, bounds.max.x - pos.x, band.x),
        (DockSide::Top, pos.y - bounds.min.y, band.y),
        (DockSide::Bottom, bounds.max.y - pos.y, band.y),
    ]
    .into_iter()
    .filter(|&(_, distance, band)| distance < band)
    .min_by(|(_, a, _), (_, b, _)| a.total_cmp(b))
    .map(|(side, _, _)| side)
}

fn find_drop_target(
    ec: EntitiesHolderRef,
    tree: &DockTree<EntityId>,
    ent_dragged: EntityId,
    mouse_pos: Vec2<f32>,
    dock: Bounds,
) -> Option<(DropTarget, Bounds)> {
    if !dock.contains(mouse_pos) {
        return None;
    }

    if let Some(side) = nearest_side(dock, mouse_pos, Vec2::splat(DOCK_EDGE_BAND)) {
        return Some(match tree.root.is_some() {
            true => (DropTarget::DockEdge(side), side_bounds(dock, side, ROOT_SPLIT_RATIO)),
            false => (DropTarget::DockFill, dock),
        });
    }

    let floating_hit = ec
        .query::<(EntityId, &WindowGroupComponent, &GlobalRectComponent)>()
        .iter()
        .filter(|(ent, group_c, rect_c)| *ent != ent_dragged && !group_c.is_docked && Bounds::of_rect(rect_c).contains(mouse_pos))
        .min_by(|(_, _, a), (_, _, b)| a.z.total_cmp(&b.z))
        .map(|(ent, _, rect_c)| (ent, Bounds::of_rect(rect_c)));

    if let Some((ent_group, group_rect)) = floating_hit {
        return center_zone(group_rect)
            .contains(mouse_pos)
            .then_some((DropTarget::GroupCenter(ent_group), group_rect));
    }

    let docked_hit = ec
        .query::<(EntityId, &WindowGroupComponent, &GlobalRectComponent)>()
        .iter()
        .filter(|(ent, group_c, rect_c)| *ent != ent_dragged && group_c.is_docked && Bounds::of_rect(rect_c).contains(mouse_pos))
        .map(|(ent, _, rect_c)| (ent, Bounds::of_rect(rect_c)))
        .next();

    let (ent_group, group_rect) = docked_hit?;
    let band = group_rect.size().map(|v| (v * GROUP_EDGE_FRACTION).min(GROUP_EDGE_MAX));

    if let Some(side) = nearest_side(group_rect, mouse_pos, band) {
        return Some((
            DropTarget::GroupEdge(ent_group, side),
            side_bounds(group_rect, side, LEAF_SPLIT_RATIO),
        ));
    }

    center_zone(group_rect)
        .contains(mouse_pos)
        .then_some((DropTarget::GroupCenter(ent_group), group_rect))
}

fn center_zone(bounds: Bounds) -> Bounds {
    let center = (bounds.min + bounds.max) * 0.5;
    let half = bounds.size().map(|v| (v * CENTER_ZONE_FRACTION).min(CENTER_ZONE_MAX) * 0.5);

    Bounds {
        min: center - half,
        max: center + half,
    }
}

fn dock_group(mut world: WorldDataMut, ent_group: EntityId, insert: impl FnOnce(&mut DockTree<EntityId>, EntityId) -> bool) {
    let ent_splitter = spawn_splitter(world.as_mut());

    let (mut res, mut ec, _evt) = world.as_tuple_mut();
    let tree = &mut res.get_mut::<DockLayoutResource>().unwrap().tree;

    let is_inserted = insert(tree, ent_splitter);

    if !is_inserted || !tree_uses_splitter(tree, ent_splitter) {
        destroy_entity_and_children(ec.as_mut(), ent_splitter);
    }

    if is_inserted && let Some(group_c) = ec.get_component_mut::<WindowGroupComponent>(ent_group) {
        group_c.is_docked = true;
    }
}

fn tree_uses_splitter(tree: &DockTree<EntityId>, ent_splitter: EntityId) -> bool {
    let mut is_used = false;

    tree.layout(Vec2::splat(0.0), Vec2::splat(1.0), 0.0, Vec2::splat(0.0), &mut |item| {
        if let DockLayoutItem::Splitter { splitter, .. } = item {
            is_used |= splitter == ent_splitter;
        }
    });

    is_used
}

fn apply_drop(mut world: WorldDataMut, ent_dragged: EntityId, target: DropTarget) {
    match target {
        DropTarget::DockFill => dock_group(world, ent_dragged, |tree, _| {
            tree.root = Some(DockNode::Leaf(ent_dragged));
            true
        }),
        DropTarget::DockEdge(side) => dock_group(world, ent_dragged, |tree, ent_splitter| {
            tree.insert_at_root(ent_dragged, side, ROOT_SPLIT_RATIO, || ent_splitter);
            true
        }),
        DropTarget::GroupEdge(ent_target, side) => dock_group(world, ent_dragged, |tree, ent_splitter| {
            tree.insert_at_leaf(ent_target, ent_dragged, side, LEAF_SPLIT_RATIO, || ent_splitter)
        }),
        DropTarget::GroupCenter(ent_target) => {
            let Some(panels) = world
                .entities()
                .get_component::<WindowGroupComponent>(ent_dragged)
                .map(|group_c| group_c.panels.clone_to_vec())
            else {
                return;
            };

            for &ent_panel in &panels {
                detach_panel(world.entities_mut(), ent_dragged, ent_panel);
                attach_panel(world.entities_mut(), ent_target, ent_panel);
            }

            if let Some(&ent_first) = panels.first() {
                set_active_panel(world.entities_mut(), ent_target, ent_first);
            }

            destroy_entity_and_children(world.entities_mut(), ent_dragged);
        }
    }
}

pub fn drag_splitter_system(
    input_res: Res<InputResource>,
    mut dock_res: ResMut<DockLayoutResource>,
    splitter_q: WorldQuery<(EntityId, &DragAreaComponent, &DockSplitterComponent)>,
) {
    let mouse_pos = Vec2::from_array(input_res.mouse.position.map(|v| v as f32));

    for (ent_splitter, drag_c, splitter_c) in splitter_q.iter() {
        continue_if_not!(Some(press_offset) = drag_c.press_global_offset);

        let axis = splitter_c.axis as usize;
        let center = mouse_pos[axis] - press_offset[axis];
        let avail = splitter_c.parent_max[axis] - splitter_c.parent_min[axis] - DOCK_GAP;

        continue_if!(avail <= 0.0);

        let ratio = ((center - splitter_c.parent_min[axis] - DOCK_GAP * 0.5) / avail).clamp(0.0, 1.0);
        dock_res.tree.set_ratio(ent_splitter, ratio);
    }
}

pub fn dock_layout_system(mut world: WorldDataMut) {
    let (res, mut ec, _evt) = world.as_tuple_mut();

    return_if_not!(Some(bounds) = current_dock_bounds(ec.as_ref()));

    let docked_z = res.get::<WindowsOrderResource>().unwrap().docked_z;

    let mut items = Vec::new();
    res.get::<DockLayoutResource>()
        .unwrap()
        .tree
        .layout(bounds.min, bounds.max, DOCK_GAP, MIN_WINDOW_SIZE, &mut |item| {
            items.push(item)
        });

    for item in items {
        match item {
            DockLayoutItem::Leaf { leaf, min, max } => {
                if let Some(rect_c) = ec.get_component_mut::<LocalRectComponent>(leaf) {
                    set_px_rect(rect_c, min, max - min);
                }
            }
            DockLayoutItem::Splitter {
                splitter,
                axis,
                min,
                max,
                parent_min,
                parent_max,
            } => {
                if let Some(rect_c) = ec.get_component_mut::<LocalRectComponent>(splitter) {
                    set_px_rect(rect_c, min, max - min);
                    rect_c.z = docked_z - 1.0;
                }

                if let Some(splitter_c) = ec.get_component_mut::<DockSplitterComponent>(splitter) {
                    *splitter_c = DockSplitterComponent {
                        axis: axis as u32,
                        parent_min,
                        parent_max,
                    };
                }

                let is_active = ec.get_component::<HoverAreaComponent>(splitter).is_some_and(|c| c.is_hovered)
                    || ec
                        .get_component::<DragAreaComponent>(splitter)
                        .is_some_and(|c| c.press_global_offset.is_some());

                if let Some(image_c) = ec.get_component_mut::<ImageComponent>(splitter) {
                    image_c.color = if is_active { SPLITTER_COLOR_HOVERED } else { SPLITTER_COLOR };
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn leaves(tree: &DockTree<u32>) -> Vec<(u32, [f32; 4])> {
        let mut out = Vec::new();
        tree.layout(Vec2::splat(0.0), Vec2::splat(100.0), 0.0, Vec2::splat(0.0), &mut |item| {
            if let DockLayoutItem::Leaf { leaf, min, max } = item {
                out.push((leaf, [min.x, min.y, max.x, max.y]));
            }
        });
        out
    }

    #[test]
    fn insert_at_root_into_empty_tree_makes_a_leaf() {
        let mut tree = DockTree::default();
        tree.insert_at_root(1, DockSide::Left, 0.3, || panic!("no splitter expected"));

        assert_eq!(tree.root, Some(DockNode::Leaf(1)));
    }

    #[test]
    fn insert_at_root_places_new_leaf_on_the_side() {
        let mut tree = DockTree::default();
        tree.insert_at_root(1, DockSide::Left, 0.25, || 0);
        tree.insert_at_root(2, DockSide::Left, 0.25, || 10);
        tree.insert_at_root(3, DockSide::Bottom, 0.5, || 11);

        assert_eq!(
            leaves(&tree),
            vec![
                (2, [0.0, 0.0, 25.0, 50.0]),
                (1, [25.0, 0.0, 100.0, 50.0]),
                (3, [0.0, 50.0, 100.0, 100.0])
            ]
        );
    }

    #[test]
    fn insert_at_leaf_splits_each_side() {
        for (side, expected) in [
            (
                DockSide::Left,
                vec![(2, [0.0, 0.0, 50.0, 100.0]), (1, [50.0, 0.0, 100.0, 100.0])],
            ),
            (
                DockSide::Right,
                vec![(1, [0.0, 0.0, 50.0, 100.0]), (2, [50.0, 0.0, 100.0, 100.0])],
            ),
            (
                DockSide::Top,
                vec![(2, [0.0, 0.0, 100.0, 50.0]), (1, [0.0, 50.0, 100.0, 100.0])],
            ),
            (
                DockSide::Bottom,
                vec![(1, [0.0, 0.0, 100.0, 50.0]), (2, [0.0, 50.0, 100.0, 100.0])],
            ),
        ] {
            let mut tree = DockTree::default();
            tree.insert_at_root(1, side, 0.5, || 0);

            assert!(tree.insert_at_leaf(1, 2, side, 0.5, || 10));
            assert_eq!(leaves(&tree), expected);
        }
    }

    #[test]
    fn insert_at_missing_leaf_fails() {
        let mut tree = DockTree::default();
        tree.insert_at_root(1, DockSide::Left, 0.5, || 0);

        assert!(!tree.insert_at_leaf(7, 2, DockSide::Left, 0.5, || 10));
        assert_eq!(tree.root, Some(DockNode::Leaf(1)));
    }

    #[test]
    fn removing_a_leaf_collapses_its_split() {
        let mut tree = DockTree::default();
        tree.insert_at_root(1, DockSide::Left, 0.5, || 0);
        tree.insert_at_leaf(1, 2, DockSide::Right, 0.5, || 10);
        tree.insert_at_leaf(2, 3, DockSide::Bottom, 0.5, || 11);

        assert_eq!(tree.remove(2), Some(Some(11)));
        assert_eq!(
            leaves(&tree),
            vec![(1, [0.0, 0.0, 50.0, 100.0]), (3, [50.0, 0.0, 100.0, 100.0])]
        );
    }

    #[test]
    fn removing_the_last_leaf_empties_the_tree() {
        let mut tree = DockTree::default();
        tree.insert_at_root(1, DockSide::Left, 0.5, || 0);
        tree.insert_at_leaf(1, 2, DockSide::Right, 0.5, || 10);

        assert_eq!(tree.remove(1), Some(Some(10)));
        assert_eq!(tree.remove(2), Some(None));
        assert_eq!(tree.root, None);
        assert_eq!(tree.remove(2), None);
    }

    #[test]
    fn ratio_is_clamped_by_min_size() {
        let mut tree = DockTree::default();
        tree.insert_at_root(1, DockSide::Left, 0.5, || 0);
        tree.insert_at_leaf(1, 2, DockSide::Right, 0.5, || 10);
        assert!(tree.set_ratio(10, 0.01));

        let mut out = Vec::new();
        tree.layout(Vec2::splat(0.0), Vec2::splat(100.0), 0.0, Vec2::splat(20.0), &mut |item| {
            if let DockLayoutItem::Leaf { leaf, max, .. } = item {
                out.push((leaf, max.x));
            }
        });

        assert_eq!(out, vec![(1, 20.0), (2, 100.0)]);
    }
}
