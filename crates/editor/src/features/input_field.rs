use std::ops::Range;

use crate::{
    features::{
        game_window::GameViewFocusResource,
        generic_dropdown::{GenericDropdownResource, select_generic_dropdown_system},
    },
    *,
};

const CARET_BLINK_HALF_PERIOD: f64 = 0.53;
const CARET_WIDTH_PX: f32 = 1.5;

pub fn register_feature(ctx: FeatureInitCtx) {
    let FeatureInitCtx {
        mut data, mut behavior, ..
    } = ctx;
    data.resources_mut().insert(SelectedInputFieldResource::default());
    data.resources_mut().insert(InputFieldContextMenuResource::default());

    let mut update = behavior.get_mut(Schedule::Update);

    update
        .group(SYSTEM_GROUP)
        .insert_child_system(select_input_field_system)
        .insert_child_system(highlight_selected_input_field_system)
        .insert_child_system(update_selected_input_field_text_system)
        .insert_child_system(open_input_field_context_menu_system)
        .insert_child_system(apply_input_field_context_menu_system)
        .insert_child_system(update_input_field_caret_system);

    update
        .order_system(check_ui_interaction_system)
        .before_system(select_input_field_system)
        .before_system(highlight_selected_input_field_system)
        .before_system(update_selected_input_field_text_system);

    update
        .order_system(select_generic_dropdown_system)
        .before_system(open_input_field_context_menu_system);

    update
        .order_system(select_generic_dropdown_system)
        .before_system(apply_input_field_context_menu_system);

    update
        .order_system(select_input_field_system)
        .before_system(apply_input_field_context_menu_system)
        .before_system(highlight_selected_input_field_system);

    update
        .order_system(select_input_field_system)
        .before_system(update_selected_input_field_text_system)
        .before_system(update_input_field_caret_system);

    update
        .order_system(apply_input_field_context_menu_system)
        .before_system(update_input_field_caret_system);
}

#[derive(Component)]
pub struct InputFieldComponent {
    pub text: EntityId,
    pub selection_border: EntityId,
    pub caret: EntityId,
    pub selection_highlight: EntityId,
}

#[derive(Resource, Default)]
pub struct SelectedInputFieldResource {
    pub selected: EntityId,
    // the field `caret`/`anchor` belong to; stays set after deselection so the context menu can still use them
    tracked: EntityId,
    caret: usize,
    anchor: usize,
    drag: Option<SelectionDrag>,
    last_activity_time: f64,
}

#[derive(Clone, Copy)]
struct SelectionDrag {
    press_char: usize,
    // a plain click into a newly selected field keeps the whole text selected until the mouse moves to another char
    is_entering: bool,
}

impl SelectedInputFieldResource {
    fn sync(&mut self, text_len: usize) {
        if self.tracked != self.selected {
            self.tracked = self.selected;
            self.select_all(text_len);
            self.drag = None;
        }

        self.caret = self.caret.min(text_len);
        self.anchor = self.anchor.min(text_len);
    }

    fn select_all(&mut self, text_len: usize) {
        self.anchor = 0;
        self.caret = text_len;
    }

    fn selection(&self) -> Range<usize> {
        self.caret.min(self.anchor)..self.caret.max(self.anchor)
    }

    fn move_caret(&mut self, to: usize, is_extending: bool) {
        self.caret = to;
        if !is_extending {
            self.anchor = to;
        }
    }

    fn replace_selection(&mut self, chars: &mut Vec<char>, insert: &[char]) {
        let range = self.selection();
        let start = range.start;
        chars.splice(range, insert.iter().copied());
        self.move_caret(start + insert.len(), false);
    }

    fn delete_to(&mut self, chars: &mut Vec<char>, to: usize) {
        if self.selection().is_empty() {
            self.anchor = to;
        }
        self.replace_selection(chars, &[]);
    }
}

#[derive(Event, Default)]
pub struct InputFieldSelectionChangedEvent;

#[derive(Event)]
pub struct InputFieldTextChangedEvent {
    pub input_field: EntityId,
}

pub fn select_input_field_system(
    input_res: Res<InputResource>,
    time_res: Res<TimeResource>,
    window_res: Res<WindowResource>,
    font_assets: Res<AssetStorageResource<Font>>,
    mut selected_input_res: ResMut<SelectedInputFieldResource>,
    mut selected_input_changed_evt: EvtMut<InputFieldSelectionChangedEvent>,
    input_field_q: WorldQuery<(EntityId, &ButtonComponent, &InputFieldComponent)>,
    text_q: WorldQuery<(&TextComponent, &GlobalRectComponent)>,
) {
    let res = &mut *selected_input_res;
    let mouse_x = input_res.mouse.position[0] as f32;

    let char_at_mouse = |field_c: &InputFieldComponent| -> Option<(usize, Vec<char>)> {
        let (text_c, rect_c) = text_q.get(field_c.text)?;
        let chars = text_c.text.chars().collect::<Vec<_>>();
        let idx = match get_text_metrics(text_c, rect_c, &window_res, &font_assets) {
            Some((left, edges)) => {
                let x = mouse_x - left;
                (0..edges.len())
                    .min_by(|&a, &b| (edges[a] - x).abs().total_cmp(&(edges[b] - x).abs()))
                    .unwrap_or(0)
            }
            None => chars.len(),
        };
        Some((idx.min(chars.len()), chars))
    };

    if input_res.mouse.is_just_pressed(MouseButton::Left) {
        res.drag = None;

        let clicked = input_field_q.iter().find(|(_, button_c, _)| button_c.was_clicked_this_frame);

        let Some((ent, button_c, field_c)) = clicked else {
            if res.selected != EntityId::EMPTY {
                res.selected = EntityId::EMPTY;
                selected_input_changed_evt.push(InputFieldSelectionChangedEvent);
            }
            return;
        };

        let Some((idx, chars)) = char_at_mouse(field_c) else {
            return;
        };

        res.last_activity_time = time_res.frame_time;

        if res.selected != ent {
            res.selected = ent;
            res.tracked = ent;
            res.select_all(chars.len());
            res.drag = Some(SelectionDrag {
                press_char: idx,
                is_entering: true,
            });
            selected_input_changed_evt.push(InputFieldSelectionChangedEvent);
            return;
        }

        res.sync(chars.len());

        if button_c.was_double_clicked_this_frame {
            let word = get_word_at(&chars, idx);
            res.anchor = word.start;
            res.caret = word.end;
            return;
        }

        res.move_caret(idx, is_shift_pressed(&input_res));
        res.drag = Some(SelectionDrag {
            press_char: idx,
            is_entering: false,
        });
        return;
    }

    if !input_res.mouse.is_pressed(MouseButton::Left) {
        res.drag = None;
        return;
    }

    let Some(drag) = res.drag else {
        return;
    };

    let Some((_, _, field_c)) = input_field_q.get(res.selected) else {
        res.drag = None;
        return;
    };

    let Some((idx, chars)) = char_at_mouse(field_c) else {
        return;
    };

    res.sync(chars.len());

    if drag.is_entering {
        if idx == drag.press_char {
            return;
        }
        res.anchor = drag.press_char;
        res.drag = Some(SelectionDrag {
            is_entering: false,
            ..drag
        });
    }

    if res.caret != idx {
        res.caret = idx;
        res.last_activity_time = time_res.frame_time;
    }
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
    key_pressed_evt: Evt<KeyPressedEvent>,
    input_res: Res<InputResource>,
    time_res: Res<TimeResource>,
    mut selected_input_res: ResMut<SelectedInputFieldResource>,
    input_field_q: WorldQuery<&InputFieldComponent>,
    mut text_q: WorldQuery<&mut TextComponent>,
    mut selected_input_changed_evt: EvtMut<InputFieldSelectionChangedEvent>,
    mut text_changed_evt: EvtMut<InputFieldTextChangedEvent>,
) {
    return_if!(key_pressed_evt.is_empty());

    let res = &mut *selected_input_res;
    let input_field = res.selected;

    let Some(input_field_c) = input_field_q.get(input_field) else {
        return;
    };

    let Some(text_c) = text_q.get_mut(input_field_c.text) else {
        return;
    };

    let mut chars = text_c.text.chars().collect::<Vec<_>>();
    res.sync(chars.len());
    res.last_activity_time = time_res.frame_time;

    let is_shift = is_shift_pressed(&input_res);
    let is_alt = input_res.keyboard.is_pressed(KeyCode::AltLeft) || input_res.keyboard.is_pressed(KeyCode::AltRight);
    let is_ctrl =
        (input_res.keyboard.is_pressed(KeyCode::ControlLeft) || input_res.keyboard.is_pressed(KeyCode::ControlRight)) && !is_alt;

    let mut is_changed = false;
    let mut is_exiting = false;

    for evt in key_pressed_evt.iter() {
        let has_selection = !res.selection().is_empty();

        match evt.key.as_ref().copied() {
            Some(KeyCode::ArrowLeft) => {
                let to = if is_ctrl {
                    get_prev_word_boundary(&chars, res.caret)
                } else if has_selection && !is_shift {
                    res.selection().start
                } else {
                    res.caret.saturating_sub(1)
                };
                res.move_caret(to, is_shift);
            }
            Some(KeyCode::ArrowRight) => {
                let to = if is_ctrl {
                    get_next_word_boundary(&chars, res.caret)
                } else if has_selection && !is_shift {
                    res.selection().end
                } else {
                    (res.caret + 1).min(chars.len())
                };
                res.move_caret(to, is_shift);
            }
            Some(KeyCode::Home) => res.move_caret(0, is_shift),
            Some(KeyCode::End) => res.move_caret(chars.len(), is_shift),
            Some(KeyCode::Backspace) => {
                let to = match is_ctrl {
                    true => get_prev_word_boundary(&chars, res.caret),
                    false => res.caret.saturating_sub(1),
                };
                res.delete_to(&mut chars, to);
                is_changed = true;
            }
            Some(KeyCode::Delete) => {
                let to = match is_ctrl {
                    true => get_next_word_boundary(&chars, res.caret),
                    false => (res.caret + 1).min(chars.len()),
                };
                res.delete_to(&mut chars, to);
                is_changed = true;
            }
            Some(KeyCode::Enter | KeyCode::NumpadEnter | KeyCode::Escape) => {
                is_exiting = true;
                break;
            }
            Some(KeyCode::KeyA) if is_ctrl => res.select_all(chars.len()),
            Some(KeyCode::KeyC) if is_ctrl => {
                if has_selection {
                    copy_to_clipboard(&String::from_iter(&chars[res.selection()]));
                }
            }
            Some(KeyCode::KeyX) if is_ctrl => {
                if has_selection {
                    copy_to_clipboard(&String::from_iter(&chars[res.selection()]));
                    res.replace_selection(&mut chars, &[]);
                    is_changed = true;
                }
            }
            Some(KeyCode::KeyV) if is_ctrl => {
                if let Some(pasted) = read_clipboard() {
                    let pasted = pasted.chars().filter(|c| !c.is_control()).collect::<Vec<_>>();
                    res.replace_selection(&mut chars, &pasted);
                    is_changed = true;
                }
            }
            _ => {
                let typed = evt.text.chars().filter(|c| !c.is_control()).collect::<Vec<_>>();
                if !typed.is_empty() {
                    res.replace_selection(&mut chars, &typed);
                    is_changed = true;
                }
            }
        }
    }

    if is_changed {
        let text = String::from_iter(&chars);
        if text_c.text.as_str() != text.as_str() {
            text_c.text = text.into();
            text_changed_evt.push(InputFieldTextChangedEvent { input_field });
        }
    }

    if is_exiting {
        res.selected = EntityId::EMPTY;
        res.drag = None;
        selected_input_changed_evt.push(InputFieldSelectionChangedEvent);
    }
}

pub fn update_input_field_caret_system(
    time_res: Res<TimeResource>,
    window_res: Res<WindowResource>,
    font_assets: Res<AssetStorageResource<Font>>,
    mut selected_input_res: ResMut<SelectedInputFieldResource>,
    input_field_q: WorldQuery<(EntityId, &InputFieldComponent)>,
    text_q: WorldQuery<(&TextComponent, &GlobalRectComponent)>,
    mut local_rect_q: WorldQuery<&mut LocalRectComponent>,
    mut disableable_q: WorldQuery<&mut LocalDisableableComponent>,
) {
    let res = &mut *selected_input_res;

    for (ent, input_field_c) in input_field_q.iter() {
        let metrics = (ent == res.selected)
            .then(|| text_q.get(input_field_c.text))
            .flatten()
            .and_then(|(text_c, rect_c)| {
                let (_, edges) = get_text_metrics(text_c, rect_c, &window_res, &font_assets)?;
                Some((text_c.text.chars().count(), edges))
            });

        let Some((text_len, edges)) = metrics else {
            for e in [input_field_c.caret, input_field_c.selection_highlight] {
                if let Some(disableable_c) = disableable_q.get_mut(e) {
                    disableable_c.is_disabled = true;
                }
            }
            continue;
        };

        res.sync(text_len);
        let selection = res.selection();
        let edge = |i: usize| edges.get(i.min(text_len)).copied().unwrap_or(0.0);

        let is_caret_visible = ((time_res.frame_time - res.last_activity_time) / CARET_BLINK_HALF_PERIOD) as u64 % 2 == 0;

        if let Some(disableable_c) = disableable_q.get_mut(input_field_c.caret) {
            disableable_c.is_disabled = !is_caret_visible;
        }
        if let Some(local_rect_c) = local_rect_q.get_mut(input_field_c.caret) {
            local_rect_c.offset.x = UiVal::px(edge(res.caret) - CARET_WIDTH_PX * 0.5);
        }

        if let Some(disableable_c) = disableable_q.get_mut(input_field_c.selection_highlight) {
            disableable_c.is_disabled = selection.is_empty();
        }
        if let Some(local_rect_c) = local_rect_q.get_mut(input_field_c.selection_highlight) {
            local_rect_c.offset.x = UiVal::px(edge(selection.start));
            local_rect_c.scale.x = Some(UiVal::px(edge(selection.end) - edge(selection.start))).into();
        }
    }
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

    let labels = InputFieldContextAction::ALL
        .iter()
        .map(|a| FfiString::from(a.label()))
        .collect();
    let position = Vec2::from_array(input_res.mouse.position.map(|v| v as f32));
    context_menu_res.request_id = generic_dropdown_res.request(labels, position);
    context_menu_res.input_field = input_field_ent;
}

pub fn apply_input_field_context_menu_system(
    time_res: Res<TimeResource>,
    generic_dropdown_res: Res<GenericDropdownResource>,
    mut context_menu_res: ResMut<InputFieldContextMenuResource>,
    mut selected_input_res: ResMut<SelectedInputFieldResource>,
    input_field_q: WorldQuery<&InputFieldComponent>,
    mut text_q: WorldQuery<&mut TextComponent>,
    mut selected_input_changed_evt: EvtMut<InputFieldSelectionChangedEvent>,
    mut text_changed_evt: EvtMut<InputFieldTextChangedEvent>,
) {
    let context_menu = &mut *context_menu_res;
    let res = &mut *selected_input_res;

    let Some(idx) = generic_dropdown_res.take_selected(&mut context_menu.request_id) else {
        return;
    };

    let Some(input_field_c) = input_field_q.get(context_menu.input_field) else {
        return;
    };

    let Some(text_c) = text_q.get_mut(input_field_c.text) else {
        return;
    };

    let mut chars = text_c.text.chars().collect::<Vec<_>>();
    // clicking the menu entry deselects the field, but its caret and selection are still tracked
    let is_tracked = res.tracked == context_menu.input_field;

    match InputFieldContextAction::ALL[idx as usize] {
        InputFieldContextAction::Copy => {
            let selection = res.selection();
            match is_tracked && !selection.is_empty() && selection.end <= chars.len() {
                true => copy_to_clipboard(&String::from_iter(&chars[selection])),
                false => copy_to_clipboard(text_c.text.as_str()),
            }
        }
        InputFieldContextAction::Paste => {
            let Some(pasted) = read_clipboard() else {
                return;
            };
            let pasted = pasted.chars().filter(|c| !c.is_control()).collect::<Vec<_>>();

            if !is_tracked {
                res.tracked = context_menu.input_field;
                res.move_caret(chars.len(), false);
            }
            res.caret = res.caret.min(chars.len());
            res.anchor = res.anchor.min(chars.len());
            res.replace_selection(&mut chars, &pasted);
            res.last_activity_time = time_res.frame_time;
            text_c.text = String::from_iter(&chars).into();

            if res.selected != context_menu.input_field {
                res.selected = context_menu.input_field;
                selected_input_changed_evt.push(InputFieldSelectionChangedEvent);
            }

            text_changed_evt.push(InputFieldTextChangedEvent {
                input_field: context_menu.input_field,
            });
        }
    }
}

// keys typed into a text field or the focused game view are not editor hotkeys
pub fn is_editor_hotkey_just_pressed(
    input_res: &InputResource,
    selected_input: &SelectedInputFieldResource,
    game_focus: &GameViewFocusResource,
    key: KeyCode,
) -> bool {
    input_res.keyboard.is_just_pressed(key) && selected_input.selected == EntityId::EMPTY && !game_focus.is_focused
}

pub fn is_editor_ctrl_hotkey_just_pressed(
    input_res: &InputResource,
    selected_input: &SelectedInputFieldResource,
    game_focus: &GameViewFocusResource,
    key: KeyCode,
) -> bool {
    let is_ctrl = input_res.keyboard.is_pressed(KeyCode::ControlLeft) || input_res.keyboard.is_pressed(KeyCode::ControlRight);
    is_ctrl && is_editor_hotkey_just_pressed(input_res, selected_input, game_focus, key)
}

fn is_shift_pressed(input_res: &InputResource) -> bool {
    input_res.keyboard.is_pressed(KeyCode::ShiftLeft) || input_res.keyboard.is_pressed(KeyCode::ShiftRight)
}

// returns the x of the text's left edge and each char's left edge relative to it, plus the text's end
fn get_text_metrics(
    text_c: &TextComponent,
    rect_c: &GlobalRectComponent,
    window_res: &WindowResource,
    font_assets: &AssetStorageResource<Font>,
) -> Option<(f32, Vec<f32>)> {
    let font = font_assets.get(&text_c.font)?;
    let window_size = Vec2::from_array(window_res.state().size.map(|v| v as f32));

    let font_size = text_c.font_size.into_px(rect_c.scale, window_size)[1];
    let horizontal_spacing = text_c.horizontal_spacing.into_px(rect_c.scale, window_size)[1];
    let scale = font.scale(font_size);

    let mut edges = vec![0.0];
    for character in text_c.text.chars() {
        edges.push(edges.last().unwrap() + font.glyph(character).advance * scale + horizontal_spacing);
    }

    Some((rect_c.center.x - rect_c.scale.x * 0.5, edges))
}

#[derive(PartialEq, Eq, Clone, Copy)]
enum CharClass {
    Space,
    Word,
    Other,
}

fn get_char_class(c: char) -> CharClass {
    match c {
        _ if c.is_whitespace() => CharClass::Space,
        _ if c.is_alphanumeric() || c == '_' => CharClass::Word,
        _ => CharClass::Other,
    }
}

fn get_prev_word_boundary(chars: &[char], from: usize) -> usize {
    let mut i = from.min(chars.len());
    while i > 0 && get_char_class(chars[i - 1]) == CharClass::Space {
        i -= 1;
    }
    let Some(class) = i.checked_sub(1).map(|p| get_char_class(chars[p])) else {
        return 0;
    };
    while i > 0 && get_char_class(chars[i - 1]) == class {
        i -= 1;
    }
    i
}

fn get_next_word_boundary(chars: &[char], from: usize) -> usize {
    let mut i = from.min(chars.len());
    if let Some(&c) = chars.get(i)
        && get_char_class(c) != CharClass::Space
    {
        let class = get_char_class(c);
        while i < chars.len() && get_char_class(chars[i]) == class {
            i += 1;
        }
    }
    while i < chars.len() && get_char_class(chars[i]) == CharClass::Space {
        i += 1;
    }
    i
}

fn get_word_at(chars: &[char], idx: usize) -> Range<usize> {
    let Some(probe) = (idx < chars.len()).then_some(idx).or(idx.checked_sub(1)) else {
        return 0..0;
    };
    let class = get_char_class(chars[probe]);

    let mut start = probe;
    while start > 0 && get_char_class(chars[start - 1]) == class {
        start -= 1;
    }
    let mut end = probe + 1;
    while end < chars.len() && get_char_class(chars[end]) == class {
        end += 1;
    }
    start..end
}
