use crate::*;

pub fn register_feature(mut world: WorldBuilderMut) {
    let mut behavior = world.behavior_mut();
    let mut update = behavior.get_mut(Schedule::Update);

    update.group(SYSTEM_GROUP)
        .insert_child_system(enable_dropdown_variants_system)
        .insert_child_system(select_dropdown_variant_system);

    update.order_system(check_ui_interaction_system)
        .before_system(enable_dropdown_variants_system)
        .before_system(select_dropdown_variant_system);
}

#[repr(C)]
#[derive(Component, Debug, Clone)]
pub struct DropdownComponent {
    pub text: EntityId,
    pub variants_container: EntityId,
}

#[derive(Component, Debug, Copy, Clone)]
pub struct DropdownEntryComponent {
    pub dropdown: EntityId,
    pub text: EntityId,
}

fn enable_dropdown_variants_system(
    dropdown_q: WorldQuery<(EntityId, &DropdownComponent, &ButtonComponent)>,
    mut dropdown_container_c: WorldQuery<(&ParentComponent, &mut LocalDisableableComponent)>,
    mut dropdown_entry_q: WorldQuery<&mut DropdownEntryComponent>,
) {
    for (dropdown_ent, dropdown_c, button_c) in dropdown_q.iter() {
        if !button_c.was_clicked_this_frame {
            continue;
        }

        let Some((variants_contianer, disableable)) = dropdown_container_c.get_mut(dropdown_c.variants_container) else {
            continue;
        };

        disableable.is_disabled = false;

        for variant_ent in &variants_contianer.children {
            if let Some(dropdown_entry_c) = dropdown_entry_q.get_mut(*variant_ent) {
                dropdown_entry_c.dropdown = dropdown_ent;
            }
        }
    }
}

pub fn select_dropdown_variant_system(
    click_evt: Evt<ButtonClickEvent>,
    dropdown_entry_q: WorldQuery<&DropdownEntryComponent>,
    dropdown_q: WorldQuery<&DropdownComponent>,
    mut text_q: WorldQuery<&mut TextComponent>,
    mut dropdown_container_c: WorldQuery<&mut LocalDisableableComponent>,
) {
    for click_evt in click_evt.iter() {
        let Some(dropdown_entry_c) = dropdown_entry_q.get(click_evt.entity) else {
            continue;
        };

        let Some(dropdown_c) = dropdown_q.get(dropdown_entry_c.dropdown) else {
            continue;
        };

        let Some(disableable_c) = dropdown_container_c.get_mut(dropdown_c.variants_container) else {
            continue;
        };

        disableable_c.is_disabled = true;

        let mut entry_text = String::new();

        if let Some(text_c) = text_q.get(dropdown_entry_c.text) {
            entry_text = text_c.text.to_string();
        }

        if let Some(text_c) = text_q.get_mut(dropdown_c.text) {
            text_c.text.clear();
            text_c.text.push_str(&entry_text);
        }
    }
}