use fruits_engine::*;

use crate::{SYSTEM_GROUP, features::ui_interaction::UiRaycastResource};

const MIN_HANDLE_HEIGHT: f32 = 20.0;

pub fn register_feature(mut world: WorldBuilderMut) {
    world
        .data_mut()
        .resources_mut()
        .insert(ScrollResource {
            active_scroll: EntityId::EMPTY,
            grab_offset: 0.0,
        });

    let mut behavior = world.behavior_mut();
    let mut update = behavior.get_mut(Schedule::Update);

    update.group(SYSTEM_GROUP)
        .insert_child_system(update_scroll_handle_size_system)
        .insert_child_system(start_end_scrolling_system)
        .insert_child_system(move_scroll_handle_system);

    update
        .order_system(update_scroll_handle_size_system)
        .before_system(start_end_scrolling_system);

    update
        .order_system(start_end_scrolling_system)
        .before_system(move_scroll_handle_system);
}

#[derive(Component)]
pub struct ScrollHandleAreaComponent {
    pub content: EntityId,
    pub handle: EntityId,
}

#[derive(Resource)]
pub struct ScrollResource {
    pub active_scroll: EntityId,
    pub grab_offset: f32,
}

fn handle_height(
    scroll_c: &ScrollHandleAreaComponent,
    area_h: f32,
    global_rect_q: &WorldQuery<&GlobalRectComponent>,
    child_q: &WorldQuery<&ChildComponent>,
) -> Option<f32> {
    let content_h = global_rect_q.get(scroll_c.content)?.scale.y;
    let view_h = global_rect_q.get(child_q.get(scroll_c.content)?.parent)?.scale.y;

    if content_h <= view_h {
        return None;
    }

    Some((area_h * view_h / content_h).max(MIN_HANDLE_HEIGHT).min(area_h))
}

fn set_progress(scroll_c: &ScrollHandleAreaComponent, progress: f32, local_rect_q: &mut WorldQuery<&mut LocalRectComponent>) {
    for ent in [scroll_c.handle, scroll_c.content] {
        if let Some(rect_c) = local_rect_q.get_mut(ent) {
            rect_c.anchor.y = progress;
            rect_c.pivot.y = progress;
        }
    }
}

fn update_scroll_handle_size_system(
    area_q: WorldQuery<(&ScrollHandleAreaComponent, EntityId)>,
    global_rect_q: WorldQuery<&GlobalRectComponent>,
    child_q: WorldQuery<&ChildComponent>,
    mut local_rect_q: WorldQuery<&mut LocalRectComponent>,
) {
    for (scroll_c, ent_area) in area_q.iter() {
        let Some(area_rect) = global_rect_q.get(ent_area) else {
            continue;
        };

        match handle_height(scroll_c, area_rect.scale.y, &global_rect_q, &child_q) {
            Some(handle_h) => {
                if let Some(handle_rect) = local_rect_q.get_mut(scroll_c.handle) {
                    handle_rect.scale.y = Some(UiVal::px(handle_h)).into();
                }
            }
            None => {
                if let Some(handle_rect) = local_rect_q.get_mut(scroll_c.handle) {
                    handle_rect.scale.y = Some(UiVal::ph(1.0)).into();
                }

                set_progress(scroll_c, 0.0, &mut local_rect_q);
            }
        }
    }
}

fn start_end_scrolling_system(
    input: Res<InputResource>,
    mut scroll_res: ResMut<ScrollResource>,
    raycast_res: Res<UiRaycastResource>,
    area_q: WorldQuery<&ScrollHandleAreaComponent>,
    global_rect_q: WorldQuery<&GlobalRectComponent>,
) {
    if input.mouse.is_just_released(MouseButton::Left) {
        scroll_res.active_scroll = EntityId::EMPTY;
    } else if input.mouse.is_just_pressed(MouseButton::Left) {
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

        return_if_not!(Some(clicked_ent) = hits.into_iter().next());

        scroll_res.active_scroll = clicked_ent;

        return_if_not!(Some(scroll_c) = area_q.get(clicked_ent));
        return_if_not!(Some(handle_rect) = global_rect_q.get(scroll_c.handle));

        let handle_top = handle_rect.center.y - handle_rect.scale.y * 0.5;
        let grab_offset = pos.y - handle_top;

        scroll_res.grab_offset = if (0.0..=handle_rect.scale.y).contains(&grab_offset) {
            grab_offset
        } else {
            handle_rect.scale.y * 0.5
        };
    }
}

fn move_scroll_handle_system(
    input: Res<InputResource>,
    scroll_res: Res<ScrollResource>,
    area_q: WorldQuery<(&ScrollHandleAreaComponent, EntityId)>,
    global_rect_q: WorldQuery<&GlobalRectComponent>,
    child_q: WorldQuery<&ChildComponent>,
    mut local_rect_q: WorldQuery<&mut LocalRectComponent>,
) {
    return_if_not!(Some((scroll_c, ent_area)) = area_q.get(scroll_res.active_scroll));
    return_if_not!(Some(area_rect) = global_rect_q.get(ent_area));
    return_if_not!(Some(handle_h) = handle_height(scroll_c, area_rect.scale.y, &global_rect_q, &child_q));

    let mouse_pos = Vec2::from_array(input.mouse.position.map(|v| v as f32));

    let area_top = area_rect.center.y - area_rect.scale.y * 0.5;
    let handle_top = mouse_pos.y - scroll_res.grab_offset;
    let track_len = area_rect.scale.y - handle_h;

    let progress = if track_len > 0.0 {
        ((handle_top - area_top) / track_len).clamp(0.0, 1.0)
    } else {
        0.0
    };

    set_progress(scroll_c, progress, &mut local_rect_q);
}
