use aphyn::*;

use crate::{
    SYSTEM_GROUP,
    features::{
        docking::remove_group_from_dock,
        inspector_window::utils::find_in_parents,
        ui_interaction::{ButtonComponent, DragAreaComponent, HoverAreaComponent, UiPressResource, check_ui_interaction_system},
    },
    prefabs::{
        TAB_COLOR_ACTIVE, TAB_COLOR_INACTIVE, WINDOW_HEADER_HEIGHT, WINDOW_TAB_STRIP_HEIGHT, WindowComponent, WindowGroupComponent,
    },
};

pub fn register_feature(ctx: FeatureInitCtx) {
    let FeatureInitCtx {
        mut data, mut behavior, ..
    } = ctx;
    data.resources_mut().insert(WindowsOrderResource::default());

    let mut update = behavior.get_mut(Schedule::Update);

    update
        .group(SYSTEM_GROUP)
        .insert_child_system(window_tabs_system)
        .insert_child_system(highlit_window_scale_handles_system)
        .insert_child_system(rescale_window_system)
        .insert_child_system(keep_windows_inside_dock_system)
        .insert_child_system(order_windows_system)
        .insert_child_system(sync_window_groups_system);

    update
        .order_system(check_ui_interaction_system)
        .before_system(window_tabs_system);
    update
        .order_system(check_ui_interaction_system)
        .before_system(highlit_window_scale_handles_system);
    update.order_system(window_tabs_system).before_system(rescale_window_system);
    update
        .order_system(rescale_window_system)
        .before_system(keep_windows_inside_dock_system);
    update
        .order_system(keep_windows_inside_dock_system)
        .before_system(order_windows_system);
    update
        .order_system(order_windows_system)
        .before_system(sync_window_groups_system);
}

pub const MIN_WINDOW_SIZE: Vec2<f32> = Vec2::new(100.0, 60.0);

pub const WINDOWS_FRONT_Z: f32 = -8_000.0;
pub const WINDOWS_Z_BAND: f32 = 1_000.0;

#[derive(Component, Default, Debug)]
pub struct DockComponent;

#[derive(Resource, Default)]
pub struct WindowsOrderResource {
    pub front_to_back: Vec<EntityId>,
    pub docked_z: f32,
}

#[derive(Clone, Copy, Debug)]
pub struct Bounds {
    pub min: Vec2<f32>,
    pub max: Vec2<f32>,
}

impl Bounds {
    const UNBOUNDED: Self = Self {
        min: Vec2::new(f32::NEG_INFINITY, f32::NEG_INFINITY),
        max: Vec2::new(f32::INFINITY, f32::INFINITY),
    };

    pub fn of_rect(rect: &GlobalRectComponent) -> Self {
        Self {
            min: rect.center - rect.scale * 0.5,
            max: rect.center + rect.scale * 0.5,
        }
    }

    pub fn size(&self) -> Vec2<f32> {
        self.max - self.min
    }

    pub fn contains(&self, pos: Vec2<f32>) -> bool {
        (0..2).all(|axis| self.min[axis] <= pos[axis] && pos[axis] <= self.max[axis])
    }
}

pub fn dock_bounds_of(rect: &GlobalRectComponent) -> Option<Bounds> {
    (rect.scale.x > 0.0 && rect.scale.y > 0.0).then(|| Bounds::of_rect(rect))
}

fn dock_bounds(dock_q: &WorldQuery<&GlobalRectComponent, WithFilter<DockComponent>>) -> Option<Bounds> {
    dock_bounds_of(dock_q.iter().next()?)
}

pub fn px_offset(rect: &LocalRectComponent, axis: usize) -> f32 {
    match rect.offset[axis] {
        UiVal { unit: UiUnit::Px, val } => val,
        _ => 0.0,
    }
}

pub fn px_scale(rect: &LocalRectComponent, axis: usize) -> f32 {
    match rect.scale[axis].into_option() {
        Some(UiVal { unit: UiUnit::Px, val }) => val,
        _ => 0.0,
    }
}

pub fn set_px_rect(rect: &mut LocalRectComponent, offset: Vec2<f32>, size: Vec2<f32>) {
    rect.offset = offset.map(UiVal::px);
    rect.scale = size.map(|v| UiVal::px(v).into());
}

pub fn attach_panel(mut ec: EntitiesHolderMut, ent_group: EntityId, ent_panel: EntityId) {
    let ent_tab = ec.get_component::<WindowComponent>(ent_panel).unwrap().tab;
    let group_c = ec.get_component_mut::<WindowGroupComponent>(ent_group).unwrap();

    group_c.panels.push(ent_panel);

    let (ent_content_area, ent_tab_strip) = (group_c.content_area, group_c.tab_strip);

    set_parent(ec.as_mut(), ent_panel, ent_content_area);
    set_parent(ec.as_mut(), ent_tab, ent_tab_strip);
}

pub fn detach_panel(mut ec: EntitiesHolderMut, ent_group: EntityId, ent_panel: EntityId) {
    let ent_tab = ec.get_component::<WindowComponent>(ent_panel).unwrap().tab;
    let group_c = ec.get_component_mut::<WindowGroupComponent>(ent_group).unwrap();

    if let Some(idx) = group_c.panels.iter().position(|&e| e == ent_panel) {
        group_c.panels.remove(idx as u64);

        if (idx as u64) < group_c.active || group_c.active >= group_c.panels.len() {
            group_c.active = group_c.active.saturating_sub(1);
        }
    }

    remove_parent(ec.as_mut(), ent_panel);
    remove_parent(ec.as_mut(), ent_tab);
}

pub fn set_active_panel(mut ec: EntitiesHolderMut, ent_group: EntityId, ent_panel: EntityId) {
    let group_c = ec.get_component_mut::<WindowGroupComponent>(ent_group).unwrap();

    if let Some(idx) = group_c.panels.iter().position(|&e| e == ent_panel) {
        group_c.active = idx as u64;
    }
}

pub fn destroy_panel(mut ec: EntitiesHolderMut, ent_panel: EntityId) {
    let ent_tab = ec.get_component::<WindowComponent>(ent_panel).unwrap().tab;

    destroy_entity_and_children(ec.as_mut(), ent_tab);
    destroy_entity_and_children(ec.as_mut(), ent_panel);
}

pub fn find_group_of_panel(ec: EntitiesHolderRef, ent_panel: EntityId) -> Option<EntityId> {
    ec.query::<(EntityId, &WindowGroupComponent)>()
        .iter()
        .find(|(_, group_c)| group_c.panels.iter().any(|&e| e == ent_panel))
        .map(|(ent_group, _)| ent_group)
}

pub fn window_tabs_system(mut world: WorldDataMut) {
    let mut to_activate = Vec::new();
    let mut to_close = Vec::new();

    {
        let ec = world.entities();

        for (ent_group, group_c) in ec.query::<(EntityId, &WindowGroupComponent)>().iter() {
            for &ent_panel in group_c.panels.iter() {
                continue_if_not!(Some(window_c) = ec.get_component::<WindowComponent>(ent_panel));

                let is_clicked = |ent| {
                    ec.get_component::<ButtonComponent>(ent)
                        .is_some_and(|b| b.was_clicked_this_frame)
                };

                if is_clicked(window_c.tab_close_button) {
                    to_close.push((ent_group, ent_panel));
                } else if is_clicked(window_c.tab) {
                    to_activate.push((ent_group, ent_panel));
                }
            }
        }
    }

    for (ent_group, ent_panel) in to_activate {
        set_active_panel(world.entities_mut(), ent_group, ent_panel);
    }

    for (ent_group, ent_panel) in to_close {
        detach_panel(world.entities_mut(), ent_group, ent_panel);
        destroy_panel(world.entities_mut(), ent_panel);

        let is_empty = world
            .entities()
            .get_component::<WindowGroupComponent>(ent_group)
            .is_some_and(|group_c| group_c.panels.len() == 0);

        if is_empty {
            remove_group_from_dock(world.as_mut(), ent_group);
            destroy_entity_and_children(world.entities_mut(), ent_group);
        }
    }
}

pub fn sync_window_groups_system(mut ec: EntitiesHolderMut) {
    let groups = ec.query::<&WindowGroupComponent>().iter().cloned().collect::<Vec<_>>();

    for group_c in groups {
        let mut tabs = Vec::new();

        for (idx, &ent_panel) in group_c.panels.iter().enumerate() {
            let is_active = idx as u64 == group_c.active;

            if let Some(disableable_c) = ec.get_component_mut::<LocalDisableableComponent>(ent_panel) {
                disableable_c.is_disabled = !is_active;
            }

            continue_if_not!(Some(ent_tab) = ec.get_component::<WindowComponent>(ent_panel).map(|window_c| window_c.tab));

            tabs.push(ent_tab);

            if let Some(image_c) = ec.get_component_mut::<ImageComponent>(ent_tab) {
                image_c.color = if is_active { TAB_COLOR_ACTIVE } else { TAB_COLOR_INACTIVE };
            }
        }

        for (idx, &ent_tab) in tabs.iter().enumerate() {
            set_parent_at(ec.as_mut(), ent_tab, group_c.tab_strip, idx);
        }

        for ent in group_c.scale_handles().into_iter().chain([group_c.header]) {
            if let Some(disableable_c) = ec.get_component_mut::<LocalDisableableComponent>(ent) {
                disableable_c.is_disabled = group_c.is_docked;
            }
        }

        let header_height = if group_c.is_docked { 0.0 } else { WINDOW_HEADER_HEIGHT };

        if let Some(rect_c) = ec.get_component_mut::<LocalRectComponent>(group_c.tab_strip) {
            rect_c.offset.y = UiVal::px(header_height);
        }

        if let Some(rect_c) = ec.get_component_mut::<LocalRectComponent>(group_c.content_area) {
            rect_c.parent_padding_min.y = UiVal::px(header_height + WINDOW_TAB_STRIP_HEIGHT);
        }
    }
}

pub fn order_windows_system(
    press_res: Res<UiPressResource>,
    mut order_res: ResMut<WindowsOrderResource>,
    mut rect_q: WorldQuery<&mut LocalRectComponent, WithFilter<WindowGroupComponent>>,
    group_q: WorldQuery<(EntityId, &WindowGroupComponent)>,
    child_q: WorldQuery<&ChildComponent>,
) {
    let is_floating = |ent: EntityId| group_q.get(ent).is_some_and(|(_, group_c)| !group_c.is_docked);

    let order = &mut order_res.front_to_back;

    order.retain(|&ent| is_floating(ent));

    for (ent, group_c) in group_q.iter() {
        if !group_c.is_docked && !order.contains(&ent) {
            order.insert(0, ent);
        }
    }

    if let Some(ent_pressed) = press_res.pressed_this_frame {
        let is_group = |e: EntityId| group_q.get(e).is_some().then_some(e);
        let ent_group = is_group(ent_pressed).or_else(|| find_in_parents(child_q, ent_pressed, is_group));

        if let Some(ent_group) = ent_group
            && let Some(idx) = order.iter().position(|&e| e == ent_group)
        {
            let ent_group = order.remove(idx);
            order.insert(0, ent_group);
        }
    }

    for (rank, &ent) in order.iter().enumerate() {
        if let Some(rect_c) = rect_q.get_mut(ent) {
            rect_c.z = WINDOWS_FRONT_Z + rank as f32 * WINDOWS_Z_BAND;
        }
    }

    let docked_z = WINDOWS_FRONT_Z + order.len() as f32 * WINDOWS_Z_BAND;
    order_res.docked_z = docked_z;

    for (ent, group_c) in group_q.iter() {
        if group_c.is_docked
            && let Some(rect_c) = rect_q.get_mut(ent)
        {
            rect_c.z = docked_z;
        }
    }
}

pub fn highlit_window_scale_handles_system(
    mut handle_q: WorldQuery<(&HoverAreaComponent, &mut ImageComponent)>,
    group_q: WorldQuery<&WindowGroupComponent>,
) {
    for group_c in group_q.iter().filter(|group_c| !group_c.is_docked) {
        for ent_handle in group_c.scale_handles() {
            continue_if_not!(Some((hover_c, image_c)) = handle_q.get_mut(ent_handle));

            image_c.color.w = match hover_c.is_hovered {
                true => 1.0,
                false => 0.0,
            }
        }
    }
}

#[derive(Clone, Copy)]
enum ScaleEdge {
    Min,
    Max,
}

pub fn rescale_window_system(
    input_res: Res<InputResource>,
    handle_q: WorldQuery<(&DragAreaComponent, &GlobalRectComponent)>,
    mut group_q: WorldQuery<(&mut LocalRectComponent, &WindowGroupComponent)>,
    dock_q: WorldQuery<&GlobalRectComponent, WithFilter<DockComponent>>,
) {
    let bounds = dock_bounds(&dock_q).unwrap_or(Bounds::UNBOUNDED);
    let mouse_pos = Vec2::from_array(input_res.mouse.position.map(|v| v as f32));

    for (group_rect, group_c) in group_q.iter_mut() {
        continue_if!(group_c.is_docked);

        let handles = [
            (group_c.scale_border_left, [Some(ScaleEdge::Min), None]),
            (group_c.scale_border_right, [Some(ScaleEdge::Max), None]),
            (group_c.scale_border_top, [None, Some(ScaleEdge::Min)]),
            (group_c.scale_border_bottom, [None, Some(ScaleEdge::Max)]),
            (group_c.scale_corner_top_left, [Some(ScaleEdge::Min), Some(ScaleEdge::Min)]),
            (group_c.scale_corner_top_right, [Some(ScaleEdge::Max), Some(ScaleEdge::Min)]),
            (group_c.scale_corner_bottom_left, [Some(ScaleEdge::Min), Some(ScaleEdge::Max)]),
            (
                group_c.scale_corner_bottom_right,
                [Some(ScaleEdge::Max), Some(ScaleEdge::Max)],
            ),
        ];

        for (handle, edges) in handles {
            continue_if_not!(Some((drag_area_c, handle_rect)) = handle_q.get(handle));
            continue_if_not!(Some(press_global_offset) = drag_area_c.press_global_offset);

            let diff = mouse_pos - press_global_offset - handle_rect.center;

            for (axis, edge) in edges.into_iter().enumerate() {
                match edge {
                    Some(ScaleEdge::Min) => drag_min_edge(group_rect, axis, diff[axis], bounds),
                    Some(ScaleEdge::Max) => drag_max_edge(group_rect, axis, diff[axis], bounds),
                    None => {}
                }
            }
        }
    }
}

fn drag_max_edge(rect: &mut LocalRectComponent, axis: usize, diff: f32, bounds: Bounds) {
    let offset = px_offset(rect, axis);
    let size = (px_scale(rect, axis) + diff)
        .min(bounds.max[axis] - offset)
        .max(MIN_WINDOW_SIZE[axis]);
    rect.scale[axis] = UiVal::px(size).into();
}

fn drag_min_edge(rect: &mut LocalRectComponent, axis: usize, diff: f32, bounds: Bounds) {
    let old_offset = px_offset(rect, axis);
    let max_edge = old_offset + px_scale(rect, axis);
    let offset = (old_offset + diff)
        .max(bounds.min[axis])
        .min(max_edge - MIN_WINDOW_SIZE[axis]);
    rect.scale[axis] = UiVal::px(max_edge - offset).into();
    rect.offset[axis] = UiVal::px(offset);
}

pub fn clamp_floating_rect(offset: Vec2<f32>, size: Vec2<f32>, bounds: Bounds) -> (Vec2<f32>, Vec2<f32>) {
    let size = Vec2::from_fn(|axis| size[axis].min(bounds.max[axis] - bounds.min[axis]).max(MIN_WINDOW_SIZE[axis]));
    let offset = Vec2::from_fn(|axis| offset[axis].min(bounds.max[axis] - size[axis]).max(bounds.min[axis]));

    (offset, size)
}

pub fn keep_windows_inside_dock_system(
    mut group_q: WorldQuery<(&mut LocalRectComponent, &WindowGroupComponent)>,
    dock_q: WorldQuery<&GlobalRectComponent, WithFilter<DockComponent>>,
) {
    return_if_not!(Some(bounds) = dock_bounds(&dock_q));

    for (rect, group_c) in group_q.iter_mut() {
        continue_if!(group_c.is_docked);

        let old_offset = Vec2::from_fn(|axis| px_offset(rect, axis));
        let old_size = Vec2::from_fn(|axis| px_scale(rect, axis));
        let (offset, size) = clamp_floating_rect(old_offset, old_size, bounds);

        for axis in 0..2 {
            if size[axis] != old_size[axis] {
                rect.scale[axis] = UiVal::px(size[axis]).into();
            }
            if offset[axis] != old_offset[axis] {
                rect.offset[axis] = UiVal::px(offset[axis]);
            }
        }
    }
}
