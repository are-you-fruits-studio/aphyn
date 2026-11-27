use aphyn::*;

use crate::{
    features::{
        input_field::InputFieldComponent,
        inspector_window::{
            FIELD_GAP, FIELD_HEIGHT, FONT_SIZE, INDENT_WIDTH,
            data::{HierarchyWindowEntryComponent, InspectorCompositeAddButton, InspectorCompositeRemoveButton},
            utils::reflection::{InspectorContainerKind, InspectorEnum, InspectorLeafKind, InspectorValue},
        },
    },
    theme::{self, PAL},
    *,
};

const CHECKBOX_SIZE: f32 = 14.0;

pub fn spawn_hierarchy_window_entry(
    mut ec: EntitiesHolderMut,
    material_text: &AssetHandle<Material>,
    material_panel: &AssetHandle<Material>,
    font: &AssetHandle<Font>,
    name: FfiString,
    parent: EntityId,
    simulated_ent: EntityId,
) -> (EntityId, EntityId) {
    let ent_entry = ec.create_entity();
    let ent_name_container = ec.create_entity();
    let ent_name = ec.create_entity();

    ec.as_mut().set_components(
        ent_entry,
        (
            GlobalRectComponent::default(),
            GlobalDisableableComponent::default(),
            LocalRectComponent {
                scale: Vec2::new(Some(UiVal::pd(1.0)).into(), None.into()),
                ..Default::default()
            },
            ParentComponent::default(),
            RectChildAlignComponent {
                anchor: Vec2::new(0.0, 0.0),
                direction: UiDirection::Vertical,
                min_gap: UiVal::px(0.0),
                spacing: UiSpacing::Chunk,
                ..Default::default()
            },
        ),
    );
    set_parent(ec.as_mut(), ent_entry, parent);

    ec.as_mut().set_components(
        ent_name_container,
        (
            GlobalRectComponent::default(),
            GlobalDisableableComponent::default(),
            LocalRectComponent {
                scale: Vec2::new(Some(UiVal::pd(1.0)).into(), Some(UiVal::px(theme::ROW_H)).into()),
                ..Default::default()
            },
            ParentComponent::default(),
            BatchedMeshComponent::default(),
            HierarchyWindowEntryComponent {
                simulated_entity: simulated_ent,
            },
            ButtonComponent::default(),
            MaterialComponent {
                material: material_panel.clone(),
            },
            ImageComponent {
                color: Vec4::splat(0.0),
                ..Default::default()
            },
        ),
    );
    set_parent(ec.as_mut(), ent_name_container, ent_entry);

    ec.as_mut().set_components(
        ent_name,
        (
            NameComponent("ent_name".into()),
            GlobalRectComponent::default(),
            GlobalDisableableComponent::default(),
            LocalRectComponent { ..Default::default() },
            BatchedMeshComponent::default(),
            MaterialComponent {
                material: material_text.clone(),
            },
            TextComponent {
                color: PAL.text_1,
                font: font.clone(),
                font_size: UiVal::px(theme::FONT),
                is_y_inverted: true,
                text: name,
                horizontal_spacing: UiVal::px(0.0),
                vertical_align: VerticalAlign::Middle,
                horizontal_align: HorizontalAlign::Left,
            },
        ),
    );
    set_parent(ec.as_mut(), ent_name, ent_name_container);

    (ent_name_container, ent_name)
}

pub fn spawn_inspector_value(
    ent: EntitiesHolderMut,
    ent_last: EntityId,
    ent_parent: EntityId,
    ent_selected_input: EntityId,
    path: ReflectPath,
    value: &InspectorValue,
    material_panel: AssetHandle<Material>,
    material_text: AssetHandle<Material>,
    font: AssetHandle<Font>,
) -> EntityId {
    match value {
        InspectorValue::Leaf { text, kind } => spawn_inspector_leaf(
            ent,
            ent_last,
            ent_parent,
            ent_selected_input,
            path,
            text.clone(),
            *kind,
            material_panel,
            material_text,
            font,
        ),
        InspectorValue::Container {
            kind,
            is_rigid,
            is_inline,
            enum_variants,
            entries,
        } => spawn_inspector_container(
            ent,
            ent_last,
            ent_parent,
            ent_selected_input,
            path,
            *kind,
            *is_rigid,
            *is_inline,
            enum_variants.as_ref(),
            entries,
            material_panel,
            material_text,
            font,
        ),
    }
}

fn spawn_inspector_leaf(
    mut ent: EntitiesHolderMut,
    ent_last: EntityId,
    ent_parent: EntityId,
    ent_selected_input: EntityId,
    path: ReflectPath,
    text: FfiString,
    kind: InspectorLeafKind,
    material_panel: AssetHandle<Material>,
    material_text: AssetHandle<Material>,
    font: AssetHandle<Font>,
) -> EntityId {
    if kind == InspectorLeafKind::Bool {
        return spawn_inspector_checkbox(ent, ent_last, ent_parent, path, text.as_str() == "true", material_panel);
    }

    let is_read_only = kind == InspectorLeafKind::ReadOnly;

    if let Some(InspectorValueComponent {
        ty: InspectorValueComponentTy::Leaf {
            text: ent_text,
            kind: last_kind,
        },
        ..
    }) = ent.get_component::<InspectorValueComponent>(ent_last)
        && (*last_kind == InspectorLeafKind::ReadOnly) == is_read_only
        && let ent_text = *ent_text
        && let Some(input_c) = ent.get_component::<InputFieldComponent>(ent_text)
        && let Some(text_c) = ent.get_component_mut::<TextComponent>(input_c.text)
    {
        if ent_text != ent_selected_input {
            text_c.text = text;
            if let Some(value_c) = ent.get_component_mut::<InspectorValueComponent>(ent_last) {
                value_c.ty = InspectorValueComponentTy::Leaf { text: ent_text, kind };
                value_c.path = path;
            }
        }
        return ent_last;
    }

    destroy_entity_and_children(ent.as_mut(), ent_last);
    let ent_text = spawn_input_area_ent(
        ent.as_mut(),
        ent_parent,
        text,
        material_panel.clone(),
        material_text.clone(),
        font.clone(),
    );

    // a read-only leaf can't be selected for editing
    if is_read_only {
        ent.remove_component::<ButtonComponent>(ent_text);
        if let Some(ent_input_text) = ent.get_component::<InputFieldComponent>(ent_text).map(|c| c.text)
            && let Some(text_c) = ent.get_component_mut::<TextComponent>(ent_input_text)
        {
            text_c.color = PAL.text_3;
        }
    }

    ent.add_component(
        ent_text,
        InspectorValueComponent {
            path,
            ty: InspectorValueComponentTy::Leaf { text: ent_text, kind },
        },
    )
    .ok()
    .unwrap();
    ent_text
}

fn spawn_inspector_checkbox(
    mut ent: EntitiesHolderMut,
    ent_last: EntityId,
    ent_parent: EntityId,
    path: ReflectPath,
    is_checked: bool,
    material_panel: AssetHandle<Material>,
) -> EntityId {
    if let Some(value_c) = ent.get_component_mut::<InspectorValueComponent>(ent_last)
        && let InspectorValueComponentTy::Checkbox {
            mark,
            is_checked: last_is_checked,
            ..
        } = &mut value_c.ty
    {
        *last_is_checked = is_checked;
        let mark = *mark;
        value_c.path = path;

        if let Some(disableable_c) = ent.get_component_mut::<LocalDisableableComponent>(mark) {
            disableable_c.is_disabled = !is_checked;
        }
        return ent_last;
    }

    destroy_entity_and_children(ent.as_mut(), ent_last);

    let ent_root = ent.create_entity();
    let ent_box = ent.create_entity();
    let ent_background = ent.create_entity();
    let ent_mark = ent.create_entity();

    ent.set_components(
        ent_root,
        (
            GlobalRectComponent::default(),
            GlobalDisableableComponent::default(),
            LocalRectComponent {
                scale: Vec2::new(UiVal::pd(1.0).into(), FIELD_HEIGHT.into()),
                ..Default::default()
            },
            InspectorValueComponent {
                path,
                ty: InspectorValueComponentTy::Checkbox {
                    button: ent_box,
                    mark: ent_mark,
                    is_checked,
                },
            },
        ),
    );
    set_parent(ent.as_mut(), ent_root, ent_parent);

    let panel = |color: Vec4<f32>| {
        (
            MaterialComponent {
                material: material_panel.clone(),
            },
            BatchedMeshComponent::default(),
            ImageComponent {
                color,
                ..Default::default()
            },
        )
    };

    ent.set_components(
        ent_box,
        (
            GlobalRectComponent::default(),
            GlobalDisableableComponent::default(),
            LocalRectComponent {
                anchor: Vec2::new(0.0, 0.5),
                pivot: Vec2::new(0.0, 0.5),
                scale: Vec2::splat(UiVal::px(CHECKBOX_SIZE)).map(Into::into),
                ..Default::default()
            },
            ButtonComponent::default(),
        ),
    );
    ent.set_components(ent_box, panel(PAL.border_default));
    set_parent(ent.as_mut(), ent_box, ent_root);

    let padded = |padding: f32| LocalRectComponent {
        parent_padding_min: Vec2::splat(UiVal::px(padding)),
        parent_padding_max: Vec2::splat(UiVal::px(padding)),
        ..Default::default()
    };

    ent.set_components(
        ent_background,
        (
            GlobalRectComponent::default(),
            GlobalDisableableComponent::default(),
            padded(1.0),
        ),
    );
    ent.set_components(ent_background, panel(PAL.surface_input));
    set_parent(ent.as_mut(), ent_background, ent_box);

    ent.set_components(
        ent_mark,
        (
            GlobalRectComponent::default(),
            GlobalDisableableComponent {
                is_disabled: !is_checked,
            },
            LocalDisableableComponent {
                is_disabled: !is_checked,
            },
            padded(3.0),
        ),
    );
    ent.set_components(ent_mark, panel(PAL.accent_fg));
    set_parent(ent.as_mut(), ent_mark, ent_background);

    ent_root
}

fn spawn_inspector_container(
    mut ent: EntitiesHolderMut,
    ent_last: EntityId,
    ent_parent: EntityId,
    ent_selected_input: EntityId,
    path: ReflectPath,
    kind: InspectorContainerKind,
    is_rigid: bool,
    is_inline: bool,
    enum_variants: Option<&InspectorEnum>,
    entries: &[(FfiString, InspectorValue)],
    material_panel: AssetHandle<Material>,
    material_text: AssetHandle<Material>,
    font: AssetHandle<Font>,
) -> EntityId {
    let (ent_root, ent_enum_meta_container, ent_fields_container, ent_container_buttons) = if let Some(InspectorValueComponent {
        ty:
            InspectorValueComponentTy::Container {
                kind: last_kind,
                is_inline: last_is_inline,
                container_enum_metadata: ent_enum_meta_container,
                container_fields: ent_fields_container,
                container_buttons: ent_container_buttons,
            },
        ..
    }) =
        ent.get_component::<InspectorValueComponent>(ent_last)
        && ent.get_component::<ParentComponent>(*ent_fields_container).is_some()
        && ent.get_component::<ParentComponent>(*ent_enum_meta_container).is_some()
        && ent.get_component::<ParentComponent>(*ent_container_buttons).is_some()
        && *last_kind == kind
        && *last_is_inline == is_inline
    {
        let containers = (
            ent_last,
            *ent_enum_meta_container,
            *ent_fields_container,
            *ent_container_buttons,
        );
        ent.get_component_mut::<InspectorValueComponent>(ent_last).unwrap().path = path.clone();
        containers
    } else {
        destroy_entity_and_children(ent.as_mut(), ent_last);

        let ent_root = spawn_default_layout_ent(ent.as_mut(), ent_parent, false);
        let ent_enum_meta_container = spawn_default_layout_ent(ent.as_mut(), ent_root, true);
        let ent_fields_container = spawn_default_layout_ent(ent.as_mut(), ent_root, !is_inline);
        let ent_container_buttons = spawn_default_layout_ent(ent.as_mut(), ent_root, true);

        if let Some(align_c) = ent.get_component_mut::<RectChildAlignComponent>(ent_container_buttons) {
            align_c.direction = UiDirection::Horizontal;
        }

        if is_inline {
            ent.get_component_mut::<RectChildAlignComponent>(ent_root).unwrap().min_gap = UiVal::px(0.0);
            ent.get_component_mut::<RectChildAlignComponent>(ent_fields_container)
                .unwrap()
                .direction = UiDirection::Horizontal;
        }

        ent.add_component(
            ent_root,
            InspectorValueComponent {
                ty: InspectorValueComponentTy::Container {
                    kind,
                    is_inline,
                    container_fields: ent_fields_container,
                    container_enum_metadata: ent_enum_meta_container,
                    container_buttons: ent_container_buttons,
                },
                path: path.clone(),
            },
        )
        .ok()
        .unwrap();

        (ent_root, ent_enum_meta_container, ent_fields_container, ent_container_buttons)
    };

    spawn_inspector_enum(
        ent.as_mut(),
        ent_enum_meta_container,
        enum_variants,
        material_panel.clone(),
        material_text.clone(),
        font.clone(),
    );

    destroy_entity_children(ent.as_mut(), ent_container_buttons);

    if !is_rigid {
        let ent_button_add = spawn_default_layout_ent(ent.as_mut(), ent_container_buttons, false);
        let ent_button_remove = spawn_default_layout_ent(ent.as_mut(), ent_container_buttons, false);

        for (ent_button, text) in [(ent_button_add, "+"), (ent_button_remove, "-")] {
            let ent_text = ent.create_entity();

            if let Some(rect_c) = ent.get_component_mut::<LocalRectComponent>(ent_button) {
                rect_c.scale = Vec2::new(UiVal::pw(0.5).into(), FIELD_HEIGHT.into());
            }
            ent.set_components(
                ent_button,
                (
                    BatchedMeshComponent::default(),
                    ButtonComponent::default(),
                    MaterialComponent {
                        material: material_panel.clone(),
                    },
                    ImageComponent {
                        is_y_inverted: true,
                        color: PAL.surface_raised,
                        ..Default::default()
                    },
                ),
            );

            ent.set_components(
                ent_text,
                (
                    GlobalRectComponent::default(),
                    GlobalDisableableComponent::default(),
                    LocalRectComponent::default(),
                    BatchedMeshComponent::default(),
                    MaterialComponent {
                        material: material_text.clone(),
                    },
                    TextComponent {
                        font: font.clone(),
                        color: PAL.text_1,
                        font_size: FONT_SIZE,
                        horizontal_align: HorizontalAlign::Middle,
                        vertical_align: VerticalAlign::Middle,
                        horizontal_spacing: UiVal::px(0.0),
                        is_y_inverted: true,
                        text: text.into(),
                    },
                ),
            );

            set_parent(ent.as_mut(), ent_text, ent_button);
        }

        ent.set_component(ent_button_add, InspectorCompositeAddButton { composite: ent_root });
        ent.set_component(ent_button_remove, InspectorCompositeRemoveButton { composite: ent_root });
    }

    let len = entries.len();

    while let Some(&ent_to_destroy) = ent
        .get_component::<ParentComponent>(ent_fields_container)
        .unwrap()
        .children()
        .get(len..)
        .and_then(|extra| extra.last())
    {
        destroy_entity_and_children(ent.as_mut(), ent_to_destroy);
    }

    for (i, (key, value)) in entries.iter().enumerate() {
        let segment = match kind {
            InspectorContainerKind::Fields => ReflectPathSegment::Field(key.clone()),
            InspectorContainerKind::List | InspectorContainerKind::Map => ReflectPathSegment::Index(i as u64),
        };

        spawn_inspector_field(
            ent.as_mut(),
            i,
            key.clone(),
            ent_fields_container,
            path.join(segment),
            value,
            ent_selected_input,
            material_panel.clone(),
            material_text.clone(),
            font.clone(),
        );
    }

    if is_inline {
        set_inline_fields_layout(ent.as_mut(), ent_fields_container);
    }

    ent_root
}

// axis cells share the row evenly; inside a cell the label is narrow so the input gets most of the width
fn set_inline_fields_layout(mut ent: EntitiesHolderMut, ent_fields_container: EntityId) {
    let fields = ent
        .get_component::<ParentComponent>(ent_fields_container)
        .map(|p| p.children().to_vec())
        .unwrap_or_default();
    let cell_width = 1.0 / fields.len().max(1) as f32;

    for ent_field in fields {
        ent.get_component_mut::<LocalRectComponent>(ent_field).unwrap().scale.x = UiVal::pd(cell_width).into();

        let Some(field_c) = ent.get_component::<InspectorFieldComponent>(ent_field).copied() else {
            continue;
        };
        let ent_key = ent.get_component::<ChildComponent>(field_c.key_text).map(|c| c.parent());

        for (ent_child, width) in [(ent_key, 0.25), (Some(field_c.value_container), 0.75)] {
            if let Some(rect_c) = ent_child.and_then(|e| ent.get_component_mut::<LocalRectComponent>(e)) {
                rect_c.scale.x = UiVal::pd(width).into();
            }
        }
    }
}

fn spawn_inspector_enum(
    mut ent: EntitiesHolderMut,
    ent_enum_meta_container: EntityId,
    enum_variants: Option<&InspectorEnum>,
    material_panel: AssetHandle<Material>,
    material_text: AssetHandle<Material>,
    font: AssetHandle<Font>,
) {
    // keep the field (and the dropdown request it holds) while only the variant text changes
    if let Some(enum_variants) = enum_variants
        && let Some(&ent_enum_field) = ent
            .get_component::<ParentComponent>(ent_enum_meta_container)
            .and_then(|p| p.children().first())
        && let Some(enum_c) = ent.get_component_mut::<InspectorEnumComponent>(ent_enum_field)
    {
        enum_c.variants = enum_variants.variants.clone();
        let ent_value_text = enum_c.value_text;
        if let Some(text_c) = ent.get_component_mut::<TextComponent>(ent_value_text) {
            text_c.text = enum_variants.variant.clone();
        }
        return;
    }

    destroy_entity_children(ent.as_mut(), ent_enum_meta_container);

    let Some(enum_variants) = enum_variants else {
        return;
    };

    let ent_enum_field = spawn_default_layout_ent(ent.as_mut(), ent_enum_meta_container, false);

    let (ent_key_enum, _) = spawn_text_ent(
        ent.as_mut(),
        ent_enum_field,
        FfiString::from("variant"),
        material_panel.clone(),
        material_text.clone(),
        font.clone(),
    );

    let ent_value_container = spawn_default_layout_ent(ent.as_mut(), ent_enum_field, false);

    let (ent_dropdown_button, ent_dropdown_text) = spawn_dropdown_button_ent(
        ent.as_mut(),
        ent_value_container,
        enum_variants.variant.clone(),
        material_panel.clone(),
        material_text.clone(),
        font.clone(),
    );

    ent.get_component_mut::<RectChildAlignComponent>(ent_enum_field)
        .unwrap()
        .direction = UiDirection::Horizontal;
    ent.get_component_mut::<LocalRectComponent>(ent_key_enum).unwrap().scale.x = UiVal::pd(0.5).into();
    ent.get_component_mut::<LocalRectComponent>(ent_value_container)
        .unwrap()
        .scale
        .x = UiVal::pd(0.5).into();

    ent.set_component(
        ent_enum_field,
        InspectorEnumComponent {
            value_button: ent_dropdown_button,
            value_text: ent_dropdown_text,
            variants: enum_variants.variants.clone(),
            dropdown_request_id: 0,
        },
    );
}

fn set_field_direction(mut ent: EntitiesHolderMut, ent_field: EntityId, is_leaf: bool) {
    let (direction, width) = match is_leaf {
        true => (UiDirection::Horizontal, 0.5),
        false => (UiDirection::Vertical, 1.0),
    };

    ent.get_component_mut::<RectChildAlignComponent>(ent_field).unwrap().direction = direction;
    if let Some(children) = ent.get_component::<ParentComponent>(ent_field).map(|p| p.children().to_vec()) {
        for ent_child in children {
            ent.get_component_mut::<LocalRectComponent>(ent_child).unwrap().scale.x = UiVal::pd(width).into();
        }
    }
}

pub fn spawn_inspector_field(
    mut ent: EntitiesHolderMut,
    i: usize,
    key: FfiString,
    ent_fields_container: EntityId,
    path: ReflectPath,
    value: &InspectorValue,
    ent_selected_input: EntityId,
    material_panel: AssetHandle<Material>,
    material_text: AssetHandle<Material>,
    font: AssetHandle<Font>,
) {
    if let Some(ent_field) = ent
        .get_component::<ParentComponent>(ent_fields_container)
        .unwrap()
        .children()
        .get(i)
        .copied()
        && let Some(field_c) = ent.get_component::<InspectorFieldComponent>(ent_field).copied()
        && let Some(text_c) = ent.get_component::<TextComponent>(field_c.key_text)
        && text_c.text.as_str() == key.as_str()
        && let Some(parent_c) = ent.get_component::<ParentComponent>(field_c.value_container)
        && parent_c.children().len() == 1
    {
        set_field_direction(ent.as_mut(), ent_field, value.is_single_line());

        let ent_child = ent
            .get_component::<ParentComponent>(field_c.value_container)
            .unwrap()
            .children()[0];
        spawn_inspector_value(
            ent.as_mut(),
            ent_child,
            field_c.value_container,
            ent_selected_input,
            path,
            value,
            material_panel,
            material_text,
            font,
        );
        return;
    }

    let ent_to_destroy = ent
        .get_component::<ParentComponent>(ent_fields_container)
        .unwrap()
        .children()
        .get(i)
        .copied()
        .unwrap_or(EntityId::EMPTY);
    destroy_entity_and_children(ent.as_mut(), ent_to_destroy);

    let ent_field = spawn_default_layout_ent(ent.as_mut(), EntityId::EMPTY, false);
    set_parent_at(ent.as_mut(), ent_field, ent_fields_container, i);

    let (ent_key, ent_key_text) = spawn_text_ent(
        ent.as_mut(),
        ent_field,
        key,
        material_panel.clone(),
        material_text.clone(),
        font.clone(),
    );

    // pressing the label drags numeric values (see `number_drag`)
    ent.add_component(ent_key, ButtonComponent::default()).ok().unwrap();

    let ent_value_container = spawn_default_layout_ent(ent.as_mut(), ent_field, false);

    if value.is_single_line() {
        set_field_direction(ent.as_mut(), ent_field, true);
    }

    spawn_inspector_value(
        ent.as_mut(),
        EntityId::EMPTY,
        ent_value_container,
        ent_selected_input,
        path,
        value,
        material_panel,
        material_text,
        font,
    );

    ent.set_component(
        ent_field,
        InspectorFieldComponent {
            key_text: ent_key_text,
            value_container: ent_value_container,
        },
    );
}

//

pub fn spawn_default_layout_ent(mut ent: EntitiesHolderMut, ent_parent: EntityId, is_padded: bool) -> EntityId {
    let entity = ent.create_entity();

    let mut local_rect = LocalRectComponent::default();
    local_rect.scale = Vec2::new(UiVal::pw(1.0).into(), None.into());
    if is_padded {
        local_rect.parent_padding_min = Vec2::new(INDENT_WIDTH, UiVal::px(0.0));
    }

    // todo: change all the "add_component" to "set_components" when the entity is created and initialized
    ent.add_component(entity, GlobalRectComponent::default()).ok().unwrap();
    ent.add_component(entity, GlobalDisableableComponent::default()).ok().unwrap();
    ent.add_component(entity, local_rect).ok().unwrap();
    ent.add_component(entity, ParentComponent::default()).ok().unwrap();
    ent.add_component(
        entity,
        RectChildAlignComponent {
            direction: UiDirection::Vertical,
            spacing: UiSpacing::Chunk,
            anchor: Vec2::new(1.0, 0.0),
            min_gap: FIELD_GAP,
        },
    )
    .ok()
    .unwrap();

    set_parent(ent.as_mut(), entity, ent_parent);

    entity
}

pub fn spawn_text_ent(
    mut ent: EntitiesHolderMut,
    ent_parent: EntityId,
    text: FfiString,
    material_panel: AssetHandle<Material>,
    material_text: AssetHandle<Material>,
    font: AssetHandle<Font>,
) -> (EntityId, EntityId) {
    let ent_root = ent.create_entity();

    let ent_text = ent.create_entity();

    ent.add_component(ent_root, GlobalRectComponent::default()).ok().unwrap();
    ent.add_component(ent_root, GlobalDisableableComponent::default())
        .ok()
        .unwrap();
    ent.add_component(
        ent_root,
        LocalRectComponent {
            scale: Vec2::new(UiVal::pd(1.0).into(), FIELD_HEIGHT.into()),
            ..Default::default()
        },
    )
    .ok()
    .unwrap();
    ent.add_component(
        ent_root,
        MaterialComponent {
            material: material_panel,
        },
    )
    .ok()
    .unwrap();
    ent.add_component(ent_root, BatchedMeshComponent::default()).ok().unwrap();
    ent.add_component(
        ent_root,
        ImageComponent {
            color: Vec4::splat(0.0),
            ..Default::default()
        },
    )
    .ok()
    .unwrap();
    // todo
    // ent.add_component(ent_root, JsonValueComponent {
    //     value_type: JsonValueComponentType::Null,
    //     data:
    // }).ok().unwrap();
    set_parent(ent.as_mut(), ent_root, ent_parent);

    ent.add_component(ent_text, GlobalRectComponent::default()).ok().unwrap();
    ent.add_component(ent_text, GlobalDisableableComponent::default())
        .ok()
        .unwrap();
    ent.add_component(ent_text, LocalRectComponent::default()).ok().unwrap();
    ent.add_component(ent_text, MaterialComponent { material: material_text })
        .ok()
        .unwrap();
    ent.add_component(ent_text, BatchedMeshComponent::default()).ok().unwrap();
    ent.add_component(
        ent_text,
        TextComponent {
            color: PAL.text_2,
            font,
            font_size: FONT_SIZE,
            is_y_inverted: true,
            text,
            horizontal_spacing: UiVal::px(0.0),
            vertical_align: VerticalAlign::Middle,
            horizontal_align: HorizontalAlign::Left,
        },
    )
    .ok()
    .unwrap();
    set_parent(ent.as_mut(), ent_text, ent_root);

    (ent_root, ent_text)
}

fn spawn_dropdown_button_ent(
    mut ent: EntitiesHolderMut,
    ent_parent: EntityId,
    variant: FfiString,
    material_panel: AssetHandle<Material>,
    material_text: AssetHandle<Material>,
    font: AssetHandle<Font>,
) -> (EntityId, EntityId) {
    let ent_root = ent.create_entity();
    let ent_text = ent.create_entity();

    ent.set_components(
        ent_root,
        (
            GlobalRectComponent::default(),
            GlobalDisableableComponent::default(),
            LocalRectComponent {
                scale: Vec2::new(UiVal::pd(1.0).into(), FIELD_HEIGHT.into()),
                ..Default::default()
            },
            ButtonComponent::default(),
            BatchedMeshComponent::default(),
            MaterialComponent {
                material: material_panel,
            },
            ImageComponent {
                color: PAL.surface_input,
                is_y_inverted: true,
                ..Default::default()
            },
        ),
    );
    set_parent(ent.as_mut(), ent_root, ent_parent);

    ent.set_components(
        ent_text,
        (
            GlobalRectComponent::default(),
            GlobalDisableableComponent::default(),
            LocalRectComponent::default(),
            BatchedMeshComponent::default(),
            MaterialComponent { material: material_text },
            TextComponent {
                color: PAL.text_1,
                font,
                font_size: FONT_SIZE,
                is_y_inverted: true,
                text: variant,
                horizontal_spacing: UiVal::px(0.0),
                vertical_align: VerticalAlign::Middle,
                horizontal_align: HorizontalAlign::Left,
            },
        ),
    );
    set_parent(ent.as_mut(), ent_text, ent_root);

    (ent_root, ent_text)
}

pub fn spawn_input_area_ent(
    mut ent: EntitiesHolderMut,
    ent_parent: EntityId,
    text: FfiString,
    material_panel: AssetHandle<Material>,
    material_text: AssetHandle<Material>,
    font: AssetHandle<Font>,
) -> EntityId {
    let ent_root = ent.create_entity();

    let ent_background = ent.create_entity();
    let ent_selection_border = ent.create_entity();
    let ent_text = ent.create_entity();
    let ent_caret = ent.create_entity();
    let ent_selection_highlight = ent.create_entity();

    ent.add_component(ent_root, GlobalRectComponent::default()).ok().unwrap();
    ent.add_component(ent_root, GlobalDisableableComponent::default())
        .ok()
        .unwrap();
    ent.add_component(
        ent_root,
        LocalRectComponent {
            scale: Vec2::new(UiVal::pd(1.0).into(), FIELD_HEIGHT.into()),
            ..Default::default()
        },
    )
    .ok()
    .unwrap();
    ent.add_component(
        ent_root,
        InputFieldComponent {
            text: ent_text,
            selection_border: ent_selection_border,
            caret: ent_caret,
            selection_highlight: ent_selection_highlight,
        },
    )
    .ok()
    .unwrap();
    ent.add_component(
        ent_root,
        MaterialComponent {
            material: material_panel.clone(),
        },
    )
    .ok()
    .unwrap();
    ent.add_component(ent_root, BatchedMeshComponent::default()).ok().unwrap();
    ent.add_component(ent_root, ButtonComponent::default()).ok().unwrap();
    ent.add_component(
        ent_root,
        ImageComponent {
            color: PAL.border_default,
            ..Default::default()
        },
    )
    .ok()
    .unwrap();
    set_parent(ent.as_mut(), ent_root, ent_parent);

    ent.add_component(ent_selection_border, GlobalRectComponent::default())
        .ok()
        .unwrap();
    ent.add_component(
        ent_selection_border,
        LocalRectComponent {
            z: -0.5,
            ..Default::default()
        },
    )
    .ok()
    .unwrap();
    ent.add_component(ent_selection_border, GlobalDisableableComponent::default())
        .ok()
        .unwrap();
    ent.add_component(ent_selection_border, LocalDisableableComponent::default())
        .ok()
        .unwrap();
    ent.add_component(
        ent_selection_border,
        MaterialComponent {
            material: material_panel.clone(),
        },
    )
    .ok()
    .unwrap();
    ent.add_component(ent_selection_border, BatchedMeshComponent::default())
        .ok()
        .unwrap();
    ent.add_component(
        ent_selection_border,
        ImageComponent {
            color: PAL.border_focus,
            ..Default::default()
        },
    )
    .ok()
    .unwrap();

    ent.add_component(ent_background, GlobalRectComponent::default())
        .ok()
        .unwrap();
    ent.add_component(ent_background, GlobalDisableableComponent::default())
        .ok()
        .unwrap();
    ent.add_component(
        ent_background,
        LocalRectComponent {
            parent_padding_min: Vec2::new(UiVal::px(1.0), UiVal::px(1.0)),
            parent_padding_max: Vec2::new(UiVal::px(1.0), UiVal::px(1.0)),
            ..Default::default()
        },
    )
    .ok()
    .unwrap();
    ent.add_component(
        ent_background,
        MaterialComponent {
            material: material_panel.clone(),
        },
    )
    .ok()
    .unwrap();
    ent.add_component(ent_background, BatchedMeshComponent::default())
        .ok()
        .unwrap();
    ent.add_component(ent_background, ChildrenRectMaskComponent).ok().unwrap();
    ent.add_component(
        ent_background,
        ImageComponent {
            color: PAL.surface_input,
            ..Default::default()
        },
    )
    .ok()
    .unwrap();

    // the input field systems move these along x and toggle their visibility
    for (ent_mark, z, height, color) in [
        (ent_selection_highlight, -0.5, 1.0, PAL.bilberry_500),
        (ent_caret, -1.5, 0.8, PAL.text_1),
    ] {
        ent.set_components(
            ent_mark,
            (
                GlobalRectComponent::default(),
                LocalRectComponent {
                    anchor: Vec2::new(0.0, 0.5),
                    pivot: Vec2::new(0.0, 0.5),
                    scale: Vec2::new(Some(UiVal::px(1.5)), Some(UiVal::ph(height))).map(Into::into),
                    z,
                    ..Default::default()
                },
                GlobalDisableableComponent { is_disabled: true },
                LocalDisableableComponent { is_disabled: true },
                MaterialComponent {
                    material: material_panel.clone(),
                },
                BatchedMeshComponent::default(),
                ImageComponent {
                    color,
                    ..Default::default()
                },
            ),
        );
    }

    ent.add_component(ent_text, GlobalRectComponent::default()).ok().unwrap();
    ent.add_component(ent_text, GlobalDisableableComponent::default())
        .ok()
        .unwrap();
    ent.add_component(ent_text, LocalRectComponent::default()).ok().unwrap();
    ent.add_component(ent_text, MaterialComponent { material: material_text })
        .ok()
        .unwrap();
    ent.add_component(ent_text, BatchedMeshComponent::default()).ok().unwrap();
    ent.add_component(
        ent_text,
        TextComponent {
            color: PAL.text_1,
            font,
            font_size: FONT_SIZE,
            is_y_inverted: true,
            text,
            horizontal_spacing: UiVal::px(0.0),
            vertical_align: VerticalAlign::Middle,
            horizontal_align: HorizontalAlign::Left,
        },
    )
    .ok()
    .unwrap();

    set_parent(ent.as_mut(), ent_selection_border, ent_root);
    set_parent(ent.as_mut(), ent_background, ent_root);
    set_parent(ent.as_mut(), ent_selection_highlight, ent_background);
    set_parent(ent.as_mut(), ent_caret, ent_background);
    set_parent(ent.as_mut(), ent_text, ent_background);

    ent_root
}

#[derive(Copy, Clone)]
pub struct SpawnedHierarchyEntry {
    pub ent_entry_with_children: EntityId,
    pub ent_entry: EntityId,
    pub ent_entry_name: EntityId,
    pub ent_children: EntityId,
    pub ent_children_container: EntityId,
}

pub fn spawn_hierarchy_entry(
    mut ec: EntitiesHolderMut,
    material_text: &AssetHandle<Material>,
    material_panel: &AssetHandle<Material>,
    font: &AssetHandle<Font>,
    parent: EntityId,
    name: FfiString,
) -> SpawnedHierarchyEntry {
    let ent_entry_with_children = ec.create_entity();
    let ent_entry = ec.create_entity();
    let ent_entry_name = ec.create_entity();

    ec.as_mut().set_components(
        ent_entry_with_children,
        (
            GlobalRectComponent::default(),
            GlobalDisableableComponent::default(),
            LocalRectComponent {
                scale: Vec2::new(Some(UiVal::pd(1.0)).into(), None.into()),
                ..Default::default()
            },
            ParentComponent::default(),
            RectChildAlignComponent {
                anchor: Vec2::new(0.0, 0.0),
                direction: UiDirection::Vertical,
                min_gap: UiVal::px(0.0),
                spacing: UiSpacing::Chunk,
                ..Default::default()
            },
        ),
    );
    set_parent(ec.as_mut(), ent_entry_with_children, parent);

    ec.as_mut().set_components(
        ent_entry,
        (
            GlobalRectComponent::default(),
            GlobalDisableableComponent::default(),
            LocalRectComponent {
                scale: Vec2::new(Some(UiVal::pd(1.0)).into(), Some(UiVal::px(theme::ROW_H)).into()),
                ..Default::default()
            },
            ParentComponent::default(),
            BatchedMeshComponent::default(),
            MaterialComponent {
                material: material_panel.clone(),
            },
            ImageComponent {
                color: Vec4::splat(0.0),
                ..Default::default()
            },
        ),
    );
    set_parent(ec.as_mut(), ent_entry, ent_entry_with_children);

    ec.as_mut().set_components(
        ent_entry_name,
        (
            NameComponent("ent_name".into()),
            GlobalRectComponent::default(),
            GlobalDisableableComponent::default(),
            LocalRectComponent { ..Default::default() },
            BatchedMeshComponent::default(),
            MaterialComponent {
                material: material_text.clone(),
            },
            TextComponent {
                color: PAL.text_1,
                font: font.clone(),
                font_size: UiVal::px(theme::FONT),
                is_y_inverted: true,
                text: name,
                horizontal_spacing: UiVal::px(0.0),
                vertical_align: VerticalAlign::Middle,
                horizontal_align: HorizontalAlign::Left,
            },
        ),
    );

    set_parent(ec.as_mut(), ent_entry_name, ent_entry);

    let ent_children = ec.create_entity();
    let ent_children_container = ec.create_entity();

    ec.as_mut().set_components(
        ent_children,
        (
            NameComponent("ent_children".into()),
            GlobalRectComponent::default(),
            GlobalDisableableComponent::default(),
            LocalRectComponent {
                scale: Vec2::new(Some(UiVal::pd(1.0)).into(), None.into()),
                ..Default::default()
            },
        ),
    );
    set_parent(ec.as_mut(), ent_children, ent_entry_with_children);

    ec.as_mut().set_components(
        ent_children_container,
        (
            GlobalRectComponent::default(),
            GlobalDisableableComponent::default(),
            LocalRectComponent {
                parent_padding_min: Vec2::new(UiVal::px(theme::INDENT), UiVal::px(0.0)),
                scale: Vec2::new(Some(UiVal::pd(1.0)).into(), None.into()),
                ..Default::default()
            },
            ParentComponent::default(),
            RectChildAlignComponent {
                anchor: Vec2::new(0.0, 0.0),
                direction: UiDirection::Vertical,
                min_gap: UiVal::px(0.0),
                spacing: UiSpacing::Chunk,
                ..Default::default()
            },
        ),
    );
    set_parent(ec.as_mut(), ent_children_container, ent_children);

    SpawnedHierarchyEntry {
        ent_entry_with_children,
        ent_entry,
        ent_entry_name,
        ent_children,
        ent_children_container,
    }
}
