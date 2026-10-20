use std::time::{Duration, Instant};

use fruits_engine::*;

use crate::SYSTEM_GROUP;

pub fn register_feature(mut world: WorldBuilderMut) {
    world
        .data_mut()
        .resources_mut()
        .insert(UiRaycastResource::default());
    world
        .data_mut()
        .resources_mut()
        .insert(DoubleClickResource::default());

    let mut behavior = world.behavior_mut();
    let mut update = behavior.get_mut(Schedule::Update);

    update
        .group(SYSTEM_GROUP)
        .insert_child_system(prepare_ui_raycast_system)
        .insert_child_system(check_ui_interaction_system);

    update
        .order_system(prepare_ui_raycast_system)
        .before_system(check_ui_interaction_system);
}

#[derive(Resource, Default)]
pub struct UiRaycastResource {
    pub bvh: Bvh<EntityId>,
}

pub const DOUBLE_CLICK_INTERVAL: Duration = Duration::from_millis(400);

#[derive(Resource, Default)]
pub struct DoubleClickResource {
    pub last_click: Option<(EntityId, Instant)>,
}

#[derive(Component, Debug, Copy, Clone, Default)]
pub struct ButtonComponent {
    pub was_clicked_this_frame: bool,
    pub was_double_clicked_this_frame: bool,
}

#[derive(Component, Debug, Copy, Clone, Default)]
pub struct DragAreaComponent {
    pub press_global_offset: Option<Vec2<f32>>,
}

#[derive(Component, Debug, Copy, Clone, Default)]
pub struct HoverAreaComponent {
    pub is_hovered: bool,
}

// Use ButtonComponent.was_clicked_this_frame instead
#[derive(Event)]
pub struct ButtonClickEvent {
    pub entity: EntityId,
}

pub fn prepare_ui_raycast_system(
    button_q: WorldQuery<
        (EntityId, &GlobalRectComponent, Option<&GlobalDisableableComponent>),
        OrFilter<(WithFilter<ButtonComponent>, WithFilter<DragAreaComponent>, WithFilter<HoverAreaComponent>)>,
    >,
    mut raycast_res: ResMut<UiRaycastResource>,
) {
    let iter = button_q.iter().filter_map(|(ent, rect, disableable)| {
        if disableable.copied().unwrap_or_default().is_disabled {
            return None;
        }

        if rect.scale.map(|v| v < 0.0).any() {
            return None;
        }

        Some((
            CollisionAabb {
                center: rect.center.xyn(rect.z),
                extents: (rect.scale * 0.5).xyn(1.0),
            }
            .into_shape(),
            ent,
        ))
    });

    raycast_res.bvh = Bvh::new(iter);
}

pub fn check_ui_interaction_system(
    rect_q: WorldQuery<&GlobalRectComponent>,
    input: Res<InputResource>,
    raycast_res: Res<UiRaycastResource>,
    mut double_click_res: ResMut<DoubleClickResource>,
    mut click_evt: EvtMut<ButtonClickEvent>,
    mut button_q: WorldQuery<&mut ButtonComponent>,
    mut drag_area_q: WorldQuery<&mut DragAreaComponent>,
    mut hover_area_q: WorldQuery<&mut HoverAreaComponent>,
) {
    for button_c in button_q.iter_mut() {
        button_c.was_clicked_this_frame = false;
        button_c.was_double_clicked_this_frame = false;
    }
    for hover_area_c in hover_area_q.iter_mut() {
        hover_area_c.is_hovered = false;
    }

    if input.mouse.is_just_released(MouseButton::Left) {
        for drag_area_c in drag_area_q.iter_mut() {
            drag_area_c.press_global_offset = None;
        }
    }

    let left_just_pressed = input.mouse.is_just_pressed(MouseButton::Left);
    let left_just_released = input.mouse.is_just_released(MouseButton::Left);

    let pos = Vec2::from_array(input.mouse.position.map(|v| v as f32));

    let mut hits = Vec::new();

    raycast_res.bvh.query(
        CollisionLine {
            bounds: LineBoundType::UNRESTRICTED,
            start: pos.xyn(0.0),
            end: pos.xyn(1.0),
        }
        .into(),
        &mut hits,
    );

    let mut min_z = f32::INFINITY;
    let mut closest_ent = None;

    for &hit in &hits {
        let Some(rect) = rect_q.get(hit) else {
            continue;
        };

        if rect.z < min_z {
            min_z = rect.z;
            closest_ent = Some(hit);
        }
    }

    let Some(target_ent) = closest_ent else {
        if left_just_pressed {
            double_click_res.last_click = None;
        }
        return;
    };

    if let Some(hover_area_c) = hover_area_q.get_mut(target_ent) {
        hover_area_c.is_hovered = true;
    }
    
    if !left_just_pressed {
        return;
    }

    let now = Instant::now();
    let is_double_click = double_click_res
        .last_click
        .is_some_and(|(ent, time)| ent == target_ent && now.duration_since(time) <= DOUBLE_CLICK_INTERVAL);

    double_click_res.last_click = if is_double_click { None } else { Some((target_ent, now)) };

    if let Some(buttom_c) = button_q.get_mut(target_ent) {
        buttom_c.was_clicked_this_frame = true;
        buttom_c.was_double_clicked_this_frame = is_double_click;
    }
    if let Some(drag_area_c) = drag_area_q.get_mut(target_ent) {
        let press_pos = Vec2::from_array(input.mouse.position.map(|v| v as f32));
        let press_global_offset = press_pos - rect_q.get(target_ent).unwrap().center;
        drag_area_c.press_global_offset = Some(press_global_offset);
    }
    click_evt.push(ButtonClickEvent { entity: target_ent });
}
