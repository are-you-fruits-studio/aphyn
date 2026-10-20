use crate::{
    features::input_field::{
        InputFieldComponent, InputFieldSelectionChangedEvent, SelectedInputFieldResource, highlight_selected_input_field_system,
        select_input_field_system,
    },
    *,
};

const DRAG_THRESHOLD_PX: f64 = 3.0;
const FLOAT_STEP_PER_PX: f64 = 0.01;
const INT_PX_PER_STEP: f64 = 4.0;

pub fn register_feature(mut world: WorldBuilderMut) {
    world.data_mut().resources_mut().insert(NumberDragResource::default());

    let mut behavior = world.behavior_mut();
    let mut update = behavior.get_mut(Schedule::Update);

    update.group(SYSTEM_GROUP).insert_child_system(drag_number_input_field_system);

    update
        .order_system(check_ui_interaction_system)
        .before_system(drag_number_input_field_system);

    update
        .order_system(select_input_field_system)
        .before_system(drag_number_input_field_system);

    update
        .order_system(drag_number_input_field_system)
        .before_system(highlight_selected_input_field_system);
}

#[derive(Resource, Default)]
pub struct NumberDragResource {
    pub drag: Option<NumberDrag>,
}

pub struct NumberDrag {
    pub input_field: EntityId,
    pub press_x: f64,
    pub last_x: f64,
    pub offset: f64,
    pub start: Option<SerializedPrimitive>,
}

#[derive(Event)]
pub struct NumberInputFieldDraggedEvent {
    pub input_field: EntityId,
}

pub fn drag_number_input_field_system(
    input_res: Res<InputResource>,
    mut drag_res: ResMut<NumberDragResource>,
    mut selected_input_res: ResMut<SelectedInputFieldResource>,
    mut selected_input_changed_evt: EvtMut<InputFieldSelectionChangedEvent>,
    mut dragged_evt: EvtMut<NumberInputFieldDraggedEvent>,
    input_field_q: WorldQuery<(EntityId, &ButtonComponent, &InputFieldComponent, &SerializedValueComponent)>,
    mut text_q: WorldQuery<&mut TextComponent>,
) {
    let mouse_x = input_res.mouse.position[0];

    if input_res.mouse.is_just_pressed(MouseButton::Left) {
        drag_res.drag = input_field_q
            .iter()
            .filter(|(_, button_c, _, serialized_value_c)| {
                button_c.was_clicked_this_frame && get_numeric_type(serialized_value_c).is_some()
            })
            .map(|(input_field, ..)| NumberDrag {
                input_field,
                press_x: mouse_x,
                last_x: mouse_x,
                offset: 0.0,
                start: None,
            })
            .next();
    }

    if !input_res.mouse.is_pressed(MouseButton::Left) {
        drag_res.drag = None;
        return;
    }

    let Some(input_field) = drag_res.drag.as_ref().map(|d| d.input_field) else {
        return;
    };

    let Some((ent_text, ty)) = input_field_q
        .get(input_field)
        .and_then(|(_, _, input_field_c, serialized_value_c)| Some((input_field_c.text, get_numeric_type(serialized_value_c)?)))
    else {
        drag_res.drag = None;
        return;
    };

    let Some(text_c) = text_q.get_mut(ent_text) else {
        drag_res.drag = None;
        return;
    };

    let drag = drag_res.drag.as_mut().unwrap();

    let start = match &drag.start {
        Some(start) => start.clone(),
        None => {
            if (mouse_x - drag.press_x).abs() < DRAG_THRESHOLD_PX {
                return;
            }

            let start = match ty {
                SerializedValuePrimitiveType::Int => SerializedPrimitive::Int(text_c.text.as_str().parse().unwrap_or(0)),
                _ => SerializedPrimitive::Float(text_c.text.as_str().parse().unwrap_or(0.0)),
            };
            drag.start = Some(start.clone());

            if selected_input_res.selected == drag.input_field {
                selected_input_res.selected = EntityId::EMPTY;
                selected_input_changed_evt.push(InputFieldSelectionChangedEvent);
            }

            start
        }
    };

    let delta_x = mouse_x - drag.last_x;
    drag.last_x = mouse_x;

    if delta_x == 0.0 {
        return;
    }

    let multiplier = if input_res.keyboard.is_pressed(KeyCode::ShiftLeft) || input_res.keyboard.is_pressed(KeyCode::ShiftRight) {
        10.0
    } else if input_res.keyboard.is_pressed(KeyCode::ControlLeft) || input_res.keyboard.is_pressed(KeyCode::ControlRight) {
        0.1
    } else {
        1.0
    };

    let text = match start {
        SerializedPrimitive::Int(start) => {
            drag.offset += delta_x * multiplier / INT_PX_PER_STEP;
            (start + drag.offset.round() as i128).to_string()
        }
        SerializedPrimitive::Float(start) => {
            drag.offset += delta_x * multiplier * FLOAT_STEP_PER_PX;
            (start + drag.offset).to_string()
        }
        _ => return,
    };

    if text_c.text.as_str() == text.as_str() {
        return;
    }

    text_c.text = text.into();
    dragged_evt.push(NumberInputFieldDraggedEvent {
        input_field: drag.input_field,
    });
}

fn get_numeric_type(serialized_value_c: &SerializedValueComponent) -> Option<SerializedValuePrimitiveType> {
    match serialized_value_c.ty {
        SerializedValueComponentTy::Primitive {
            ty: ty @ (SerializedValuePrimitiveType::Int | SerializedValuePrimitiveType::Float),
            ..
        } => Some(ty),
        _ => None,
    }
}
