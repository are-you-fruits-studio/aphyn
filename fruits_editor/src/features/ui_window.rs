use fruits_engine::*;

use crate::{SYSTEM_GROUP, features::ui_interaction::{ButtonComponent, DragAreaComponent, HoverAreaComponent, check_ui_interaction_system}, prefabs::WindowComponent};

pub fn register_feature(mut world: WorldBuilderMut) {
    let mut behavior = world.behavior_mut();
    let mut update = behavior.get_mut(Schedule::Update);

    update
        .group(SYSTEM_GROUP)
        .insert_child_system(drag_window_system)
        .insert_child_system(close_window_system)
        .insert_child_system(highlit_window_scale_handles_system)
        .insert_child_system(rescale_window_system);

    update
        .order_system(check_ui_interaction_system)
        .before_system(close_window_system)
        .before_system(drag_window_system)
        .before_system(highlit_window_scale_handles_system)
        .before_system(rescale_window_system);
}

pub fn drag_window_system(
    input_res: Res<InputResource>,
    mut window_q: WorldQuery<(&mut LocalRectComponent, &WindowComponent)>,
    drag_area_q: WorldQuery<(&DragAreaComponent, &GlobalRectComponent)>,
) {
    for (window_local_rect, window_c) in window_q.iter_mut() {
        let Some((drag_area_c, drag_area_rect)) = drag_area_q.get(window_c.drag_header) else {
            continue;
        };

        let Some(press_global_offset) = drag_area_c.press_global_offset else {
            continue;
        };

        let expected_pos = Vec2::from_array(input_res.mouse.position.map(|v| v as f32)) - press_global_offset;
        let real_pos = drag_area_rect.center;
        let old_offset = window_local_rect.offset.map(|u| match u.unit {
            UiUnit::Px => u.val,
            _ => 0.0,
        });
        window_local_rect.offset = (old_offset + expected_pos -  real_pos).map(UiVal::px);
    }
}

pub fn close_window_system(mut world: WorldDataMut) {
    let mut entities_to_delete = Vec::new();

    for (ent_window, window_c) in world.entities().query::<(EntityId, &WindowComponent)>().iter() {
        let Some(button_c) = world.entities().get_component::<ButtonComponent>(window_c.close_button) else {
            continue;
        };

        if button_c.was_clicked_this_frame {
            entities_to_delete.push(ent_window);
        }
    }

    for ent_to_delete in entities_to_delete {
        destroy_entity_and_children(world.entities_mut(), ent_to_delete);
    }
}

pub fn highlit_window_scale_handles_system(
    mut handle_q: WorldQuery<(&HoverAreaComponent, &mut ImageComponent)>,
    window_q: WorldQuery<&WindowComponent>,
) {
    for window_c in window_q.iter() {
        let ent_scale_borders = [
            window_c.scale_border_left,
            window_c.scale_border_right,
            window_c.scale_border_top,
            window_c.scale_border_bottom,
        ];

        for ent_scale_border in ent_scale_borders {
            let Some((hover_c, image_c)) = handle_q.get_mut(ent_scale_border) else {
                continue;
            };
            
            image_c.color.w = match hover_c.is_hovered {
                true => 1.0,
                false => 0.0,
            }
        }
    }
}

pub fn rescale_window_system(
    input_res: Res<InputResource>,
    handle_q: WorldQuery<(&DragAreaComponent, &GlobalRectComponent)>,
    mut window_q: WorldQuery<(&mut LocalRectComponent, &WindowComponent)>,
) {
    for (window_rect, window_c) in window_q.iter_mut() {
        'side: {
            if let Some((drag_area_c, handle_rect)) = handle_q.get(window_c.scale_border_right) {
                let Some(press_global_offset) = drag_area_c.press_global_offset else {
                    break 'side;
                };
                
                let expected_pos = input_res.mouse.position[0] as f32 - press_global_offset.x;
                let real_pos = handle_rect.center.x;
                let old_scale = match window_rect.scale.x.into_option() {
                    Some(UiVal { unit: UiUnit::Px, val }) => val,
                    _ => 0.0,
                };
                window_rect.scale.x = UiVal::px(old_scale + expected_pos - real_pos).into();
            }
        }
        'side: {
            if let Some((drag_area_c, handle_rect)) = handle_q.get(window_c.scale_border_left) {
                let Some(press_global_offset) = drag_area_c.press_global_offset else {
                    break 'side;
                };
                
                let expected_pos = input_res.mouse.position[0] as f32 - press_global_offset.x;
                let real_pos = handle_rect.center.x;
                let diff = expected_pos - real_pos;
                let old_scale = match window_rect.scale.x.into_option() {
                    Some(UiVal { unit: UiUnit::Px, val }) => val,
                    _ => 0.0,
                };
                let old_offset = match window_rect.offset.x {
                    UiVal { unit: UiUnit::Px, val } => val,
                    _ => 0.0,
                };
                window_rect.scale.x = UiVal::px(old_scale - diff).into();
                window_rect.offset.x = UiVal::px(old_offset + diff).into();
            }
        }
        'side: {
            if let Some((drag_area_c, handle_rect)) = handle_q.get(window_c.scale_border_bottom) {
                let Some(press_global_offset) = drag_area_c.press_global_offset else {
                    break 'side;
                };
                
                let expected_pos = input_res.mouse.position[1] as f32 - press_global_offset.y;
                let real_pos = handle_rect.center.y;
                let old_scale = match window_rect.scale.y.into_option() {
                    Some(UiVal { unit: UiUnit::Px, val }) => val,
                    _ => 0.0,
                };
                window_rect.scale.y = UiVal::px(old_scale + expected_pos - real_pos).into();
            }
        }
        'side: {
            if let Some((drag_area_c, handle_rect)) = handle_q.get(window_c.scale_border_top) {
                let Some(press_global_offset) = drag_area_c.press_global_offset else {
                    break 'side;
                };
                
                let expected_pos = input_res.mouse.position[1] as f32 - press_global_offset.y;
                let real_pos = handle_rect.center.y;
                let diff = expected_pos - real_pos;
                let old_scale = match window_rect.scale.y.into_option() {
                    Some(UiVal { unit: UiUnit::Px, val }) => val,
                    _ => 0.0,
                };
                let old_offset = match window_rect.offset.y {
                    UiVal { unit: UiUnit::Px, val } => val,
                    _ => 0.0,
                };
                window_rect.scale.y = UiVal::px(old_scale - diff).into();
                window_rect.offset.y = UiVal::px(old_offset + diff).into();
            }
        }
    }
}