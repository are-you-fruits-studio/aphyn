use crate::{features::generic_dropdown::{GenericDropdownResource, select_generic_dropdown_system}, *};

pub fn register_feature(mut world: WorldBuilderMut) {
    world.data_mut().resources_mut().insert(SelectedInputFieldResource::default());
    world.data_mut().resources_mut().insert(InputFieldContextMenuResource::default());

    let mut behavior = world.behavior_mut();
    let mut update = behavior.get_mut(Schedule::Update);

    update.group(SYSTEM_GROUP)
        .insert_child_system(select_input_field_system)
        .insert_child_system(highlight_selected_input_field_system)
        .insert_child_system(update_selected_input_field_text_system)
        .insert_child_system(open_input_field_context_menu_system)
        .insert_child_system(apply_input_field_context_menu_system);

    update.order_system(check_ui_interaction_system)
        .before_system(select_input_field_system)
        .before_system(highlight_selected_input_field_system)
        .before_system(update_selected_input_field_text_system);

    update.order_system(select_generic_dropdown_system)
        .before_system(open_input_field_context_menu_system);

    update.order_system(select_generic_dropdown_system)
        .before_system(apply_input_field_context_menu_system);

    update.order_system(select_input_field_system)
        .before_system(apply_input_field_context_menu_system)
        .before_system(highlight_selected_input_field_system);
}

#[derive(Component)]
pub struct InputFieldComponent {
    pub text: EntityId,
    pub selection_border: EntityId,
}

#[derive(Resource, Default)]
pub struct SelectedInputFieldResource {
    pub selected: EntityId,
}

#[derive(Event, Default)]
pub struct InputFieldSelectionChangedEvent;

#[derive(Event)]
pub struct InputFieldTextChangedEvent {
    pub input_field: EntityId,
}

pub fn select_input_field_system(
    input_res: Res<InputResource>,
    mut selected_input_res: ResMut<SelectedInputFieldResource>,
    mut selected_input_changed_evt: EvtMut<InputFieldSelectionChangedEvent>,
    input_field_q: WorldQuery<(EntityId, &ButtonComponent), WithFilter<InputFieldComponent>>,
) {
    if input_res.mouse.is_just_pressed(MouseButton::Left) && selected_input_res.selected != EntityId::EMPTY {
        selected_input_res.selected = EntityId::EMPTY;
        selected_input_changed_evt.push(InputFieldSelectionChangedEvent);
    }

    let Some(input_field_ent) = input_field_q.iter().filter(|(e, b)| b.was_clicked_this_frame).map(|(e, b)| e).next() else {
        return;
    };

    selected_input_res.selected = input_field_ent;
    selected_input_changed_evt.push(InputFieldSelectionChangedEvent);
}

pub fn highlight_selected_input_field_system(
    selected_input_res: Res<SelectedInputFieldResource>,
    input_field_q: WorldQuery<(&InputFieldComponent, EntityId)>,
    mut disableable_q: WorldQuery<&mut LocalDisableableComponent>,
) {
    for (input_field_c, ent) in input_field_q.iter() {
        let Some(disableable_c) = disableable_q.get_mut(input_field_c.selection_border) else {
            continue;
        };

        disableable_c.is_disabled = selected_input_res.selected != ent;
    }
}

pub fn update_selected_input_field_text_system(
    text_input_evt: Evt<TextInputEvent>,
    selected_input_res: Res<SelectedInputFieldResource>,
    input_field_q: WorldQuery<&InputFieldComponent>,
    mut text_q: WorldQuery<&mut TextComponent>,
    mut text_changed_evt: EvtMut<InputFieldTextChangedEvent>,
) {
    let Some(input_field_c) = input_field_q.get(selected_input_res.selected) else {
        return;
    };

    let Some(text_c) = text_q.get_mut(input_field_c.text) else {
        return;
    };

    if text_input_evt.is_empty() {
        return;
    }

    for evt in text_input_evt.iter() {
        if evt.0.as_str() == "\u{8}" {
            let mut text = text_c.text.to_string();
            text.pop();
            text_c.text = text.into();
        } else {
            let mut text = text_c.text.to_string();
            text.push_str(&evt.0);
            text_c.text = text.into();
        }
    }

    text_changed_evt.push(InputFieldTextChangedEvent { input_field: selected_input_res.selected });
}

#[derive(Clone, Copy)]
enum InputFieldContextAction {
    Copy,
    Paste,
}

impl InputFieldContextAction {
    const ALL: [Self; 2] = [Self::Copy, Self::Paste];

    fn label(self) -> &'static str {
        match self {
            Self::Copy => "Copy",
            Self::Paste => "Paste",
        }
    }
}

#[derive(Resource, Default)]
pub struct InputFieldContextMenuResource {
    request_id: u64,
    input_field: EntityId,
}

pub fn open_input_field_context_menu_system(
    input_res: Res<InputResource>,
    input_field_q: WorldQuery<(EntityId, &ButtonComponent), WithFilter<InputFieldComponent>>,
    mut generic_dropdown_res: ResMut<GenericDropdownResource>,
    mut context_menu_res: ResMut<InputFieldContextMenuResource>,
) {
    let Some((input_field_ent, _)) = input_field_q.iter().find(|(_, b)| b.was_right_clicked_this_frame) else {
        return;
    };

    let labels = InputFieldContextAction::ALL.iter().map(|a| FfiString::from(a.label())).collect();
    let position = Vec2::from_array(input_res.mouse.position.map(|v| v as f32));
    context_menu_res.request_id = generic_dropdown_res.request(labels, position);
    context_menu_res.input_field = input_field_ent;
}

pub fn apply_input_field_context_menu_system(
    generic_dropdown_res: Res<GenericDropdownResource>,
    mut context_menu_res: ResMut<InputFieldContextMenuResource>,
    mut selected_input_res: ResMut<SelectedInputFieldResource>,
    input_field_q: WorldQuery<&InputFieldComponent>,
    mut text_q: WorldQuery<&mut TextComponent>,
    mut selected_input_changed_evt: EvtMut<InputFieldSelectionChangedEvent>,
    mut text_changed_evt: EvtMut<InputFieldTextChangedEvent>,
) {
    let context_menu = &mut *context_menu_res;

    let Some(idx) = generic_dropdown_res.take_selected(&mut context_menu.request_id) else {
        return;
    };

    let Some(input_field_c) = input_field_q.get(context_menu.input_field) else {
        return;
    };

    let Some(text_c) = text_q.get_mut(input_field_c.text) else {
        return;
    };

    match InputFieldContextAction::ALL[idx as usize] {
        InputFieldContextAction::Copy => copy_to_clipboard(text_c.text.as_str()),
        InputFieldContextAction::Paste => {
            let Some(pasted) = read_clipboard() else {
                return;
            };

            let mut text = text_c.text.to_string();
            text.extend(pasted.chars().filter(|c| *c != '\n' && *c != '\r'));
            text_c.text = text.into();

            if selected_input_res.selected != context_menu.input_field {
                selected_input_res.selected = context_menu.input_field;
                selected_input_changed_evt.push(InputFieldSelectionChangedEvent);
            }

            text_changed_evt.push(InputFieldTextChangedEvent { input_field: context_menu.input_field });
        }
    }
}
