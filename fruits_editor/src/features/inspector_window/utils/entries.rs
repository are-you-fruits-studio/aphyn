use fruits_engine::*;

use crate::{
    features::{
        dropdown::{DropdownComponent, DropdownEntryComponent}, input_field::InputFieldComponent, inspector_window::{
            FIELD_GAP, FIELD_HEIGHT, FONT_SIZE, INDENT_WIDTH, data::{HierarchyWindowEntryComponent, SerializedCompositeAddButton, SerializedCompositeRemoveButton},
        },
    }, *,
};

pub fn spawn_hierarchy_window_entry(
    mut ec: EntitiesHolderMut,
    material_text: &AssetHandle<StandardMaterial>,
    material_panel: &AssetHandle<StandardMaterial>,
    font: &AssetHandle<Font>,
    name: FfiString,
    parent: EntityId,
    simulated_ent: EntityId,
 ) {
    let ent_entry = ec.create_entity();
    let ent_name_container = ec.create_entity();
    let ent_name = ec.create_entity();

    ec.as_mut().set_components(ent_entry, (
        GlobalRectComponent::default(),
        LocalRectComponent {
            scale: Vec2::new(Some(UiVal::pd(1.0)).into(), None.into()),
            ..Default::default()
        },
        ChildComponent { parent: parent },
        ParentComponent { children: vec![].into() },
        RectChildAlignComponent {
            anchor: Vec2::new(0.0, 0.0),
            direction: UiDirection::Vertical,
            min_gap: UiVal::px(0.0),
            spacing: UiSpacing::Chunk,
            ..Default::default()
        },
    ));
    ec.get_component_mut::<ParentComponent>(parent).unwrap().children.push(ent_entry);

    ec.as_mut().set_components(ent_name_container, (
        GlobalRectComponent::default(),
        LocalRectComponent {
            scale: Vec2::new(Some(UiVal::pd(1.0)).into(), Some(UiVal::px(20.0)).into()),
            ..Default::default()
        },
        ChildComponent { parent: ent_entry },
        ParentComponent { children: vec![].into() },
        BatchedMeshComponent::default(),
        HierarchyWindowEntryComponent { simulated_entity: simulated_ent },
        ButtonComponent::default(),
        StandardMaterialComponent { material: material_panel.clone() },
        ImageComponent {
            color: Vec4::splat(0.0),
            ..Default::default()
        },
    ));
    ec.get_component_mut::<ParentComponent>(ent_entry).unwrap().children.push(ent_name_container);

    ec.as_mut().set_components(ent_name, (
        DebugNameComponent("ent_name".into()),
        GlobalRectComponent::default(),
        LocalRectComponent {
            ..Default::default()
        },
        ChildComponent { parent: ent_name_container },
        BatchedMeshComponent::default(),
        StandardMaterialComponent { material: material_text.clone() },
        TextComponent {
            color: Vec4::from_array(parse_color_rgba_f32("#000000ff").unwrap()),
            font: font.clone(),
            font_size: UiVal::px(18.0),
            is_y_inverted: true,
            text: name,
            horizontal_spacing: UiVal::px(0.0),
            vertical_align: VerticalAlign::Middle,
            horizontal_align: HorizontalAlign::Left,
        },
    ));
}

pub fn parse_serialized(ent: EntitiesHolderRef, ent_target: EntityId) -> SerializedValue {
    let Some(serialized_value_component) = ent.get_component::<SerializedValueComponent>(ent_target) else {
        return SerializedValue::Null;
    };

    match &serialized_value_component.ty {
        SerializedValueComponentTy::Primitive { text, ty } => {
            let default_result = match ty {
                SerializedValuePrimitiveType::Null => SerializedValue::Null,
                SerializedValuePrimitiveType::Bool => SerializedValue::Primitive(SerializedPrimitive::Bool(false)),
                SerializedValuePrimitiveType::Int => SerializedValue::Primitive(SerializedPrimitive::Int(0)),
                SerializedValuePrimitiveType::Float => SerializedValue::Primitive(SerializedPrimitive::Float(0.0)),
                SerializedValuePrimitiveType::String => SerializedValue::Primitive(SerializedPrimitive::String(FfiString::new())),
            };

            let Some(input_c) = ent.get_component::<InputFieldComponent>(*text) else {
                return default_result;
            };

            let Some(text_c) = ent.get_component::<TextComponent>(input_c.text) else {
                return default_result;
            };

            match ty {
                SerializedValuePrimitiveType::Null => SerializedValue::Null,
                SerializedValuePrimitiveType::Bool => SerializedValue::Primitive(SerializedPrimitive::Bool(
                    text_c.text.as_str() == "true" || text_c.text.as_str() == "True",
                )),
                SerializedValuePrimitiveType::Int => {
                    SerializedValue::Primitive(SerializedPrimitive::Int(text_c.text.as_str().parse().unwrap_or(0)))
                }
                SerializedValuePrimitiveType::Float => {
                    SerializedValue::Primitive(SerializedPrimitive::Float(text_c.text.as_str().parse().unwrap_or(0.0)))
                }
                SerializedValuePrimitiveType::String => {
                    SerializedValue::Primitive(SerializedPrimitive::String(text_c.text.as_str().into()))
                }
            }
        }
        SerializedValueComponentTy::Container {
            ty,
            container_enum_metadata: ent_enum_meta_container,
            container_fields: container,
            ..
        } => {
            let default_result = match ty {
                SerializedValueContainerType::List => SerializedValue::Composite(SerializedComposite {
                    is_rigid: false,
                    values: SerializedCompositeValues::List(FfiVec::new()),
                }),
                SerializedValueContainerType::Map => SerializedValue::Composite(SerializedComposite {
                    is_rigid: false,
                    values: SerializedCompositeValues::Map(SerializedMap::default()),
                }),
            };

            let Some(container_parent_c) = ent.get_component::<ParentComponent>(*container) else {
                return default_result;
            };

            let Some(enum_meta_parent_c) = ent.get_component::<ParentComponent>(*ent_enum_meta_container) else {
                return default_result;
            };

            let mut enum_metadata = None;

            if let Some(ent_enum_meta) = enum_meta_parent_c.children.get(0) {
                if let Some(enum_metadata_c) = ent.get_component::<SerializedEnumMetadataComponent>(*ent_enum_meta) {
                    if let Some(dropdown_c) = ent.get_component::<DropdownComponent>(enum_metadata_c.value_text) {
                        if let Some(text_c) = ent.get_component::<TextComponent>(dropdown_c.text) {
                            enum_metadata = Some(SerializedEnumMetadata {
                                variant: text_c.text.clone(),
                                variants: enum_metadata_c.variants.clone(),
                            });
                        };
                    };
                }
            }

            let mut values = Vec::new();

            for child in &container_parent_c.children {
                let Some(field_c) = ent.get_component::<SerializedFieldComponent>(*child) else {
                    continue;
                };

                let Some(value_parent_c) = ent.get_component::<ParentComponent>(field_c.value_container) else {
                    continue;
                };

                let Some(value_ent) = value_parent_c.children.first() else {
                    continue;
                };

                let value = parse_serialized(ent, *value_ent);

                let key = match ent.get_component::<InputFieldComponent>(field_c.key_text) {
                    Some(input_c) => match ent.get_component::<TextComponent>(input_c.text) {
                        Some(key_text_c) => key_text_c.text.clone(),
                        None => FfiString::new(),
                    },
                    None => match ent.get_component::<TextComponent>(field_c.key_text) {
                        Some(key_text_c) => key_text_c.text.clone(),
                        None => FfiString::new(),
                    },
                };

                values.push((key, value));
            }

            match ty {
                SerializedValueContainerType::List => SerializedValue::Composite(SerializedComposite {
                    is_rigid: false,
                    values: SerializedCompositeValues::List(values.into_iter().map(|(k, v)| v).collect()),
                }),
                SerializedValueContainerType::Map => SerializedValue::Composite(SerializedComposite {
                    is_rigid: false,
                    values: SerializedCompositeValues::Map({
                        let mut map = SerializedMap::default();

                        map.enum_metadata = enum_metadata.into();

                        for (key, value) in values {
                            map.values.insert(key, value);
                        }

                        map
                    }),
                }),
            }
        }
    }
}

pub fn spawn_serialized(
    mut ent: EntitiesHolderMut,
    ent_last: EntityId,
    ent_parent: EntityId,
    ent_selected_input: EntityId,
    serializer_path: FfiString,
    serialized: &SerializedValue,
    material_panel: AssetHandle<StandardMaterial>,
    material_text: AssetHandle<StandardMaterial>,
    font: AssetHandle<Font>,
) -> EntityId {
    match serialized {
        SerializedValue::Null => spawn_default_layout_ent(ent, ent_parent, false),
        SerializedValue::Primitive(serialized) => spawn_serialized_primitive(
            ent,
            ent_last,
            ent_parent,
            ent_selected_input,
            serializer_path,
            serialized,
            material_panel,
            material_text,
            font,
        ),
        SerializedValue::Composite(serialized) => spawn_serialized_composite(
            ent,
            ent_last,
            ent_parent,
            ent_selected_input,
            serializer_path,
            serialized,
            material_panel,
            material_text,
            font,
        ),
    }
}

fn spawn_serialized_primitive(
    mut ent: EntitiesHolderMut,
    ent_last: EntityId,
    ent_parent: EntityId,
    ent_selected_input: EntityId,
    serializer_path: FfiString,
    serialized: &SerializedPrimitive,
    material_panel: AssetHandle<StandardMaterial>,
    material_text: AssetHandle<StandardMaterial>,
    font: AssetHandle<Font>,
) -> EntityId {
    let (text, ty) = match serialized {
        SerializedPrimitive::Bool(serialized) => (serialized.to_string().into(), SerializedValuePrimitiveType::Bool),
        SerializedPrimitive::Int(serialized) => (serialized.to_string().into(), SerializedValuePrimitiveType::Int),
        SerializedPrimitive::Float(serialized) => (serialized.to_string().into(), SerializedValuePrimitiveType::Float),
        SerializedPrimitive::String(serialized) => (serialized.to_string().into(), SerializedValuePrimitiveType::String),
    };

    if let Some(SerializedValueComponent { path, ty: SerializedValueComponentTy::Primitive { text: ent_text, .. } }) = ent.get_component::<SerializedValueComponent>(ent_last)
        && let ent_text = *ent_text
        && let Some(input_c) = ent.get_component::<InputFieldComponent>(ent_text)
        && let Some(text_c) = ent.get_component_mut::<TextComponent>(input_c.text)
    {
        if ent_text != ent_selected_input {
            text_c.text = text;
            if let Some(SerializedValueComponent { path, ty: SerializedValueComponentTy::Primitive { ty: ent_ty, .. }}) =
                ent.get_component_mut::<SerializedValueComponent>(ent_last)
            {
                *ent_ty = ty;
                *path = serializer_path;
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

    ent.add_component(ent_text, SerializedValueComponent { path: serializer_path, ty: SerializedValueComponentTy::Primitive { text: ent_text, ty }})
        .ok()
        .unwrap();
    ent_text
}

fn spawn_serialized_composite(
    mut ent: EntitiesHolderMut,
    ent_last: EntityId,
    ent_parent: EntityId,
    ent_selected_input: EntityId,
    serializer_path: FfiString,
    serialized_composite: &SerializedComposite,
    material_panel: AssetHandle<StandardMaterial>,
    material_text: AssetHandle<StandardMaterial>,
    font: AssetHandle<Font>,
) -> EntityId {
    let expected_ty = match &serialized_composite.values {
        SerializedCompositeValues::Map(_) => SerializedValueContainerType::Map,
        SerializedCompositeValues::List(_) => SerializedValueContainerType::List,
    };

    let (ent_root, ent_enum_meta_container, ent_fields_container, ent_container_buttons) =
        if let Some(SerializedValueComponent { path, ty: SerializedValueComponentTy::Container {
            ty,
            container_enum_metadata: ent_enum_meta_container,
            container_fields: ent_fields_container,
            container_buttons: ent_container_buttons,
        }}) = ent.get_component::<SerializedValueComponent>(ent_last)
            && ent.get_component::<ParentComponent>(*ent_fields_container).is_some()
            && ent.get_component::<ParentComponent>(*ent_enum_meta_container).is_some()
            && ent.get_component::<ParentComponent>(*ent_container_buttons).is_some()
            && *ty == expected_ty
        {
            (ent_last, *ent_enum_meta_container, *ent_fields_container, *ent_container_buttons)
        } else {
            destroy_entity_and_children(ent.as_mut(), ent_last);

            let ent_root = spawn_default_layout_ent(ent.as_mut(), ent_parent, false);
            let ent_enum_meta_container = spawn_default_layout_ent(ent.as_mut(), ent_root, true);
            let ent_fields_container = spawn_default_layout_ent(ent.as_mut(), ent_root, true);
            let ent_container_buttons = spawn_default_layout_ent(ent.as_mut(), ent_root, true);

            if let Some(align_c) = ent.get_component_mut::<RectChildAlignComponent>(ent_container_buttons) {
                align_c.direction = UiDirection::Horizontal;
            }

            ent.add_component(
                ent_root,
                SerializedValueComponent {
                    ty: SerializedValueComponentTy::Container {
                        container_fields: ent_fields_container,
                        ty: expected_ty,
                        container_enum_metadata: ent_enum_meta_container,
                        container_buttons: ent_container_buttons,
                    },
                    path: serializer_path.clone(),
                },
            )
            .ok()
            .unwrap();

            (ent_root, ent_enum_meta_container, ent_fields_container, ent_container_buttons)
        };

    match &serialized_composite.values {
        SerializedCompositeValues::Map(serialized_map) => {
            spawn_serialized_enum_metadata(
                ent.as_mut(),
                ent_enum_meta_container,
                serialized_map.enum_metadata.as_ref(),
                material_panel.clone(),
                material_text.clone(),
                font.clone(),
            );
        }
        SerializedCompositeValues::List(_) => {
            destroy_entity_children(ent.as_mut(), ent_enum_meta_container);
        }
    }

    destroy_entity_children(ent.as_mut(), ent_container_buttons);

    if !serialized_composite.is_rigid {
        let ent_button_add = spawn_default_layout_ent(ent.as_mut(), ent_container_buttons, false);
        let ent_button_remove = spawn_default_layout_ent(ent.as_mut(), ent_container_buttons, false);

        for (ent_button, text) in [(ent_button_add, "+"), (ent_button_remove, "-")] {
            let ent_text = ent.create_entity();

            if let Some(rect_c) = ent.get_component_mut::<LocalRectComponent>(ent_button) {
                rect_c.scale = Vec2::new(UiVal::pw(0.5).into(), FIELD_HEIGHT.into());
            }
            ent.add_component(ent_button, BatchedMeshComponent::default()).ok().unwrap();
            ent.add_component(ent_button, ButtonComponent::default()).ok().unwrap();
            ent.add_component(
                ent_button,
                StandardMaterialComponent {
                    material: material_panel.clone(),
                },
            )
            .ok()
            .unwrap();
            ent.add_component(
                ent_button,
                ImageComponent {
                    is_y_inverted: true,
                    color: Vec4::new(0.5, 0.5, 0.5, 1.0),
                    ..Default::default()
                },
            )
            .ok()
            .unwrap();

            ent.add_component(ent_text, GlobalRectComponent::default()).ok().unwrap();
            ent.add_component(ent_text, LocalRectComponent::default()).ok().unwrap();
            ent.add_component(ent_text, ChildComponent { parent: ent_button })
                .ok()
                .unwrap();
            ent.add_component(ent_text, BatchedMeshComponent::default()).ok().unwrap();
            ent.add_component(
                ent_text,
                StandardMaterialComponent {
                    material: material_text.clone(),
                },
            )
            .ok()
            .unwrap();
            ent.add_component(
                ent_text,
                TextComponent {
                    font: font.clone(),
                    color: Vec4::new(1.0, 1.0, 1.0, 1.0),
                    font_size: FONT_SIZE,
                    horizontal_align: HorizontalAlign::Middle,
                    vertical_align: VerticalAlign::Middle,
                    horizontal_spacing: UiVal::px(0.0),
                    is_y_inverted: true,
                    text: text.into(),
                },
            )
            .ok()
            .unwrap();

            if let Some(parent_c) = ent.get_component_mut::<ParentComponent>(ent_button) {
                parent_c.children.push(ent_text);
            }
        }

        ent.add_component(ent_button_add, SerializedCompositeAddButton { composite: ent_root })
            .ok()
            .unwrap();
        ent.add_component(ent_button_remove, SerializedCompositeRemoveButton { composite: ent_root })
            .ok()
            .unwrap();
    }

    let len = match &serialized_composite.values {
        SerializedCompositeValues::Map(serialized_map) => serialized_map.values.len(),
        SerializedCompositeValues::List(serialized_list) => serialized_list.len(),
    };

    while ent
        .get_component_mut::<ParentComponent>(ent_fields_container)
        .unwrap()
        .children
        .len()
        > len
    {
        let parent_c = ent.get_component_mut::<ParentComponent>(ent_fields_container).unwrap();
        let ent_to_destroy = parent_c.children.pop().unwrap();
        destroy_entity_and_children(ent.as_mut(), ent_to_destroy);
    }

    for i in 0..len {
        let (serialized_key, serialized_val) = match &serialized_composite.values {
            SerializedCompositeValues::Map(serialized_map) => {
                serialized_map.values.get_by_idx(i).map(|(k, v)| (k.clone(), v)).unwrap()
            }
            SerializedCompositeValues::List(serialized_list) => (FfiString::from(i.to_string()), &serialized_list[i]),
        };

        let field_path = format!("{serializer_path}/{serialized_key}").into();

        spawn_serialized_field(
            ent.as_mut(),
            i,
            serialized_key,
            ent_fields_container,
            field_path,
            serialized_val,
            ent_selected_input,
            material_panel.clone(),
            material_text.clone(),
            font.clone(),
        );
    }

    ent_root
}

fn spawn_serialized_enum_metadata(
    mut ent: EntitiesHolderMut,
    ent_enum_meta_container: EntityId,
    serialized_enum_metadata: Option<&SerializedEnumMetadata>,
    material_panel: AssetHandle<StandardMaterial>,
    material_text: AssetHandle<StandardMaterial>,
    font: AssetHandle<Font>,
) {
    destroy_entity_children(ent.as_mut(), ent_enum_meta_container);

    if let Some(serialized_enum_metadata) = serialized_enum_metadata {
        let ent_enum_field = spawn_default_layout_ent(ent.as_mut(), ent_enum_meta_container, false);

        let (ent_key_enum, ent_key_text_enum) = spawn_text_ent(
            ent.as_mut(),
            ent_enum_field,
            String::from("$enum_variant").into(),
            material_panel.clone(),
            material_text.clone(),
            font.clone(),
        );

        let ent_value_container = spawn_default_layout_ent(ent.as_mut(), ent_enum_field, false);

        let enum_metadata_input_ent = spawn_dropdown_ent(
            ent.as_mut(),
            ent_value_container,
            serialized_enum_metadata.variant.clone().into(),
            serialized_enum_metadata.variants.clone(),
            material_panel.clone(),
            material_text.clone(),
            font.clone(),
        );

        {
            ent.get_component_mut::<RectChildAlignComponent>(ent_enum_field)
                .unwrap()
                .direction = UiDirection::Horizontal;
            ent.get_component_mut::<LocalRectComponent>(ent_key_enum).unwrap().scale.x = UiVal::pd(0.5).into();
            ent.get_component_mut::<LocalRectComponent>(ent_value_container)
                .unwrap()
                .scale
                .x = UiVal::pd(0.5).into();
        }

        ent.add_component(
            ent_enum_field,
            SerializedEnumMetadataComponent {
                value_text: enum_metadata_input_ent,
                variants: serialized_enum_metadata.variants.clone(),
            },
        )
        .ok()
        .unwrap();
    }
}

pub fn spawn_serialized_field(
    mut ent: EntitiesHolderMut,
    i: u64,
    serialized_key: FfiString,
    ent_fields_container: EntityId,
    serializer_path: FfiString,
    serialized_val: &SerializedValue,
    ent_selected_input: EntityId,
    material_panel: AssetHandle<StandardMaterial>,
    material_text: AssetHandle<StandardMaterial>,
    font: AssetHandle<Font>,
) {
    if let Some(ent_field) = ent
        .get_component_mut::<ParentComponent>(ent_fields_container)
        .unwrap()
        .children
        .get(i)
        .copied()
        && let Some(field_c) = ent.get_component::<SerializedFieldComponent>(ent_field).copied()
        && let Some(text_c) = ent.get_component::<TextComponent>(field_c.key_text)
        && text_c.text.as_str() == serialized_key.as_str()
        && let Some(parent_c) = ent.get_component::<ParentComponent>(field_c.value_container)
        && parent_c.children.len() == 1
    {
        if matches!(serialized_val, SerializedValue::Primitive { .. } | SerializedValue::Null) {
            ent.get_component_mut::<RectChildAlignComponent>(ent_field).unwrap().direction = UiDirection::Horizontal;
            if let Some(parent_c) = ent.get_component::<ParentComponent>(ent_field).cloned() {
                for ent_child in parent_c.children {
                    ent.get_component_mut::<LocalRectComponent>(ent_child).unwrap().scale.x = UiVal::pd(0.5).into();
                }
            }
        } else {
            ent.get_component_mut::<RectChildAlignComponent>(ent_field).unwrap().direction = UiDirection::Vertical;
            if let Some(parent_c) = ent.get_component::<ParentComponent>(ent_field).cloned() {
                for ent_child in parent_c.children {
                    ent.get_component_mut::<LocalRectComponent>(ent_child).unwrap().scale.x = UiVal::pd(1.0).into();
                }
            }
        }

        let parent_c = ent.get_component::<ParentComponent>(field_c.value_container).unwrap();
        let ent_child = parent_c.children[0];
        spawn_serialized(
            ent.as_mut(),
            ent_child,
            field_c.value_container,
            ent_selected_input,
            serializer_path,
            serialized_val,
            material_panel.clone(),
            material_text.clone(),
            font.clone(),
        );
        return;
    }

    let parent_c = ent.get_component_mut::<ParentComponent>(ent_fields_container).unwrap();
    let ent_to_destroy = parent_c.children.get(i).copied().unwrap_or(EntityId::EMPTY);
    destroy_entity_and_children(ent.as_mut(), ent_to_destroy);

    let ent_field = spawn_default_layout_ent(ent.as_mut(), EntityId::EMPTY, false);
    let parent_c = ent.get_component_mut::<ParentComponent>(ent_fields_container).unwrap();

    if let Some(child) = parent_c.children.get_mut(i) {
        *child = ent_field;
    } else {
        parent_c.children.push(ent_field);
    }
    ent.get_component_mut::<ChildComponent>(ent_field).unwrap().parent = ent_fields_container;

    let (ent_key, ent_key_text) = spawn_text_ent(
        ent.as_mut(),
        ent_field,
        serialized_key,
        material_panel.clone(),
        material_text.clone(),
        font.clone(),
    );

    let ent_value_container = spawn_default_layout_ent(ent.as_mut(), ent_field, false);

    if matches!(serialized_val, SerializedValue::Primitive { .. } | SerializedValue::Null) {
        ent.get_component_mut::<RectChildAlignComponent>(ent_field).unwrap().direction = UiDirection::Horizontal;
        ent.get_component_mut::<LocalRectComponent>(ent_key).unwrap().scale.x = UiVal::pd(0.5).into();
        ent.get_component_mut::<LocalRectComponent>(ent_value_container)
            .unwrap()
            .scale
            .x = UiVal::pd(0.5).into();
    }

    spawn_serialized(
        ent.as_mut(),
        EntityId::EMPTY,
        ent_value_container,
        ent_selected_input,
        serializer_path,
        serialized_val,
        material_panel.clone(),
        material_text.clone(),
        font.clone(),
    );

    ent.add_component(
        ent_field,
        SerializedFieldComponent {
            key_text: ent_key_text,
            value_container: ent_value_container,
        },
    )
    .ok()
    .unwrap();
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
    ent.add_component(entity, local_rect).ok().unwrap();
    ent.add_component(entity, ChildComponent { parent: ent_parent }).ok().unwrap();
    ent.add_component(entity, ParentComponent { children: vec![].into() })
        .ok()
        .unwrap();
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

    if let Some(parent_c) = ent.get_component_mut::<ParentComponent>(ent_parent) {
        parent_c.children.push(entity);
    }

    entity
}

pub fn spawn_text_ent(
    mut ent: EntitiesHolderMut,
    ent_parent: EntityId,
    text: FfiString,
    material_panel: AssetHandle<StandardMaterial>,
    material_text: AssetHandle<StandardMaterial>,
    font: AssetHandle<Font>,
) -> (EntityId, EntityId) {
    let ent_root = ent.create_entity();

    let ent_text = ent.create_entity();

    ent.add_component(ent_root, GlobalRectComponent::default()).ok().unwrap();
    ent.add_component(
        ent_root,
        LocalRectComponent {
            scale: Vec2::new(UiVal::pd(1.0).into(), FIELD_HEIGHT.into()),
            ..Default::default()
        },
    )
    .ok()
    .unwrap();
    ent.add_component(ent_root, ChildComponent { parent: ent_parent })
        .ok()
        .unwrap();
    ent.add_component(
        ent_root,
        StandardMaterialComponent {
            material: material_panel,
        },
    )
    .ok()
    .unwrap();
    ent.add_component(ent_root, BatchedMeshComponent::default()).ok().unwrap();
    ent.add_component(
        ent_root,
        ImageComponent {
            color: Vec4::from_array(rgba_f32_array!("#38383880")),
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
    ent.get_component_mut::<ParentComponent>(ent_parent)
        .unwrap()
        .children
        .push(ent_root);

    ent.add_component(ent_text, GlobalRectComponent::default()).ok().unwrap();
    ent.add_component(ent_text, LocalRectComponent::default()).ok().unwrap();
    ent.add_component(ent_text, ChildComponent { parent: ent_root }).ok().unwrap();
    ent.add_component(ent_text, StandardMaterialComponent { material: material_text })
        .ok()
        .unwrap();
    ent.add_component(ent_text, BatchedMeshComponent::default()).ok().unwrap();
    ent.add_component(
        ent_text,
        TextComponent {
            color: Vec4::from_array(parse_color_rgba_f32("#000000ff").unwrap()),
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

    (ent_root, ent_text)
}

fn spawn_dropdown_ent(
    mut ent: EntitiesHolderMut,
    ent_parent: EntityId,
    variant: FfiString,
    variants: FfiVec<FfiString>,
    material_panel: AssetHandle<StandardMaterial>,
    material_text: AssetHandle<StandardMaterial>,
    font: AssetHandle<Font>,
) -> EntityId {
    let ent_root = ent.create_entity();
    let ent_active_variant = ent.create_entity();
    let ent_variants_container = ent.create_entity();

    ent.add_component(ent_root, GlobalRectComponent::default()).ok().unwrap();
    ent.add_component(
        ent_root,
        LocalRectComponent {
            scale: Vec2::new(UiVal::pd(1.0).into(), FIELD_HEIGHT.into()),
            ..Default::default()
        },
    )
    .ok()
    .unwrap();
    ent.add_component(ent_root, ChildComponent { parent: ent_parent })
        .ok()
        .unwrap();
    ent.add_component(ent_root, ParentComponent::default()).ok().unwrap();
    ent.add_component(ent_root, ButtonComponent::default()).ok().unwrap();
    ent.add_component(ent_root, BatchedMeshComponent::default()).ok().unwrap();
    ent.add_component(
        ent_root,
        StandardMaterialComponent {
            material: material_panel.clone(),
        },
    )
    .ok()
    .unwrap();
    ent.add_component(
        ent_root,
        ImageComponent {
            color: Vec4::new(1.0, 1.0, 1.0, 1.0),
            is_y_inverted: true,
            ..Default::default()
        },
    )
    .ok()
    .unwrap();
    ent.add_component(
        ent_root,
        DropdownComponent {
            text: ent_active_variant,
            variants_container: ent_variants_container,
        },
    )
    .ok()
    .unwrap();
    if let Some(parent_c) = ent.get_component_mut::<ParentComponent>(ent_parent) {
        parent_c.children.push(ent_root);
    }

    ent.add_component(ent_active_variant, GlobalRectComponent::default())
        .ok()
        .unwrap();
    ent.add_component(
        ent_active_variant,
        LocalRectComponent {
            scale: Vec2::new(UiVal::pd(1.0).into(), FIELD_HEIGHT.into()),
            ..Default::default()
        },
    )
    .ok()
    .unwrap();
    ent.add_component(ent_active_variant, ChildComponent { parent: ent_root })
        .ok()
        .unwrap();
    ent.add_component(ent_active_variant, BatchedMeshComponent::default())
        .ok()
        .unwrap();
    ent.add_component(
        ent_active_variant,
        StandardMaterialComponent {
            material: material_text.clone(),
        },
    )
    .ok()
    .unwrap();
    ent.add_component(
        ent_active_variant,
        TextComponent {
            color: Vec4::from_array(parse_color_rgba_f32("#000000ff").unwrap()),
            font: font.clone(),
            font_size: FONT_SIZE,
            is_y_inverted: true,
            text: variant,
            horizontal_spacing: UiVal::px(0.0),
            vertical_align: VerticalAlign::Middle,
            horizontal_align: HorizontalAlign::Left,
        },
    )
    .ok()
    .unwrap();
    if let Some(parent_c) = ent.get_component_mut::<ParentComponent>(ent_root) {
        parent_c.children.push(ent_active_variant);
    }

    ent.add_component(ent_variants_container, GlobalRectComponent::default())
        .ok()
        .unwrap();
    ent.add_component(
        ent_variants_container,
        LocalRectComponent {
            scale: Vec2::new(UiVal::pd(1.0).into(), None.into()),
            anchor: Vec2::new(0.0, 1.0),
            pivot: Vec2::new(0.0, 0.0),
            z: -20.0,
            ..Default::default()
        },
    )
    .ok()
    .unwrap();
    ent.add_component(ent_variants_container, ChildComponent { parent: ent_root })
        .ok()
        .unwrap();
    ent.add_component(ent_variants_container, ParentComponent::default())
        .ok()
        .unwrap();
    ent.add_component(ent_variants_container, GlobalDisableableComponent::default())
        .ok()
        .unwrap();
    ent.add_component(ent_variants_container, LocalDisableableComponent { is_disabled: true })
        .ok()
        .unwrap();
    ent.add_component(ent_variants_container, BatchedMeshComponent::default())
        .ok()
        .unwrap();
    ent.add_component(
        ent_variants_container,
        StandardMaterialComponent {
            material: material_panel.clone(),
        },
    )
    .ok()
    .unwrap();
    // todo: remove?
    ent.add_component(
        ent_variants_container,
        ImageComponent {
            color: Vec4::new(1.0, 0.0, 1.0, 1.0),
            is_y_inverted: true,
            ..Default::default()
        },
    )
    .ok()
    .unwrap();
    ent.add_component(
        ent_variants_container,
        RectChildAlignComponent {
            direction: UiDirection::Vertical,
            anchor: Vec2::new(0.0, 0.0),
            min_gap: FIELD_GAP,
            spacing: UiSpacing::Chunk,
        },
    )
    .ok()
    .unwrap();
    if let Some(parent_c) = ent.get_component_mut::<ParentComponent>(ent_root) {
        parent_c.children.push(ent_variants_container);
    }

    // todo

    for variant_text in variants {
        let ent_variant = ent.create_entity();
        let ent_variant_text = ent.create_entity();

        ent.add_component(ent_variant, GlobalRectComponent::default()).ok().unwrap();
        ent.add_component(
            ent_variant,
            LocalRectComponent {
                scale: Vec2::new(UiVal::pd(1.0).into(), FIELD_HEIGHT.into()),
                ..Default::default()
            },
        )
        .ok()
        .unwrap();
        ent.add_component(
            ent_variant,
            ChildComponent {
                parent: ent_variants_container,
            },
        )
        .ok()
        .unwrap();
        ent.add_component(ent_variant, ParentComponent::default()).ok().unwrap();
        ent.add_component(ent_variant, GlobalDisableableComponent::default())
            .ok()
            .unwrap();
        ent.add_component(ent_variant, LocalDisableableComponent::default())
            .ok()
            .unwrap();
        ent.add_component(ent_variant, ButtonComponent::default()).ok().unwrap();
        // ent.add_component(ent_variant, BatchedMeshComponent::default()).ok().unwrap();
        // ent.add_component(ent_variant, StandardMaterialComponent { material: material_panel.clone() }).ok().unwrap();
        // ent.add_component(ent_variant, ImageComponent {
        //     color: Vec4::new(1.0, 1.0, 1.0, 1.0),
        //     is_y_inverted: true,
        //     ..Default::default()
        // }).ok().unwrap();
        ent.add_component(
            ent_variant,
            DropdownEntryComponent {
                dropdown: ent_root,
                text: ent_variant_text,
            },
        )
        .ok()
        .unwrap();
        if let Some(parent_c) = ent.get_component_mut::<ParentComponent>(ent_variants_container) {
            parent_c.children.push(ent_variant);
        }

        ent.add_component(ent_variant_text, GlobalRectComponent::default())
            .ok()
            .unwrap();
        ent.add_component(
            ent_variant_text,
            LocalRectComponent {
                scale: Vec2::new(UiVal::pd(1.0).into(), FIELD_HEIGHT.into()),
                ..Default::default()
            },
        )
        .ok()
        .unwrap();
        ent.add_component(ent_variant_text, ChildComponent { parent: ent_variant })
            .ok()
            .unwrap();
        ent.add_component(ent_variant_text, GlobalDisableableComponent::default())
            .ok()
            .unwrap();
        ent.add_component(ent_variant_text, LocalDisableableComponent::default())
            .ok()
            .unwrap();
        ent.add_component(ent_variant_text, BatchedMeshComponent::default())
            .ok()
            .unwrap();
        ent.add_component(
            ent_variant_text,
            StandardMaterialComponent {
                material: material_text.clone(),
            },
        )
        .ok()
        .unwrap();
        ent.add_component(
            ent_variant_text,
            TextComponent {
                color: Vec4::from_array(parse_color_rgba_f32("#000000ff").unwrap()),
                font: font.clone(),
                font_size: FONT_SIZE,
                is_y_inverted: true,
                text: variant_text,
                horizontal_spacing: UiVal::px(0.0),
                vertical_align: VerticalAlign::Middle,
                horizontal_align: HorizontalAlign::Left,
            },
        )
        .ok()
        .unwrap();
        if let Some(parent_c) = ent.get_component_mut::<ParentComponent>(ent_variant) {
            parent_c.children.push(ent_variant_text);
        }
        // todo
    }

    ent_root
}

pub fn spawn_input_area_ent(
    mut ent: EntitiesHolderMut,
    ent_parent: EntityId,
    text: FfiString,
    material_panel: AssetHandle<StandardMaterial>,
    material_text: AssetHandle<StandardMaterial>,
    font: AssetHandle<Font>,
) -> EntityId {
    let ent_root = ent.create_entity();

    let ent_background = ent.create_entity();
    let ent_selection_border = ent.create_entity();
    let ent_text = ent.create_entity();

    ent.add_component(ent_root, GlobalRectComponent::default()).ok().unwrap();
    ent.add_component(
        ent_root,
        LocalRectComponent {
            scale: Vec2::new(UiVal::pd(1.0).into(), FIELD_HEIGHT.into()),
            ..Default::default()
        },
    )
    .ok()
    .unwrap();
    ent.add_component(ent_root, ChildComponent { parent: ent_parent })
        .ok()
        .unwrap();
    ent.add_component(
        ent_root,
        InputFieldComponent {
            text: ent_text,
            selection_border: ent_selection_border,
        },
    )
    .ok()
    .unwrap();
    ent.add_component(
        ent_root,
        StandardMaterialComponent {
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
            color: Vec4::from_array(rgba_f32_array!("#a7a7a7ff")),
            ..Default::default()
        },
    )
    .ok()
    .unwrap();
    ent.get_component_mut::<ParentComponent>(ent_parent)
        .unwrap()
        .children
        .push(ent_root);

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
    ent.add_component(ent_selection_border, ChildComponent { parent: ent_root })
        .ok()
        .unwrap();
    ent.add_component(
        ent_selection_border,
        StandardMaterialComponent {
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
            color: Vec4::from_array(rgba_f32_array!("#5c66ebff")),
            ..Default::default()
        },
    )
    .ok()
    .unwrap();

    ent.add_component(ent_background, GlobalRectComponent::default())
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
    ent.add_component(ent_background, ChildComponent { parent: ent_root })
        .ok()
        .unwrap();
    ent.add_component(
        ent_background,
        StandardMaterialComponent {
            material: material_panel,
        },
    )
    .ok()
    .unwrap();
    ent.add_component(ent_background, BatchedMeshComponent::default())
        .ok()
        .unwrap();
    ent.add_component(
        ent_background,
        ImageComponent {
            color: Vec4::from_array(rgba_f32_array!("#272727ff")),
            ..Default::default()
        },
    )
    .ok()
    .unwrap();

    ent.add_component(ent_text, GlobalRectComponent::default()).ok().unwrap();
    ent.add_component(ent_text, LocalRectComponent::default()).ok().unwrap();
    ent.add_component(ent_text, ChildComponent { parent: ent_background })
        .ok()
        .unwrap();
    ent.add_component(ent_text, StandardMaterialComponent { material: material_text })
        .ok()
        .unwrap();
    ent.add_component(ent_text, BatchedMeshComponent::default()).ok().unwrap();
    ent.add_component(
        ent_text,
        TextComponent {
            color: Vec4::from_array(parse_color_rgba_f32("#ffffffff").unwrap()),
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
    material_text: &AssetHandle<StandardMaterial>,
    material_panel: &AssetHandle<StandardMaterial>,
    font: &AssetHandle<Font>,
    parent: EntityId,
    name: FfiString,
) -> SpawnedHierarchyEntry {
    let ent_entry_with_children = ec.create_entity();
    let ent_entry = ec.create_entity();
    let ent_entry_name = ec.create_entity();

    ec.as_mut().set_components(ent_entry_with_children, (
        GlobalRectComponent::default(),
        LocalRectComponent {
            scale: Vec2::new(Some(UiVal::pd(1.0)).into(), None.into()),
            ..Default::default()
        },
        ChildComponent { parent },
        ParentComponent { children: vec![].into() },
        RectChildAlignComponent {
            anchor: Vec2::new(0.0, 0.0),
            direction: UiDirection::Vertical,
            min_gap: UiVal::px(0.0),
            spacing: UiSpacing::Chunk,
            ..Default::default()
        },
    ));
    ec.get_component_mut::<ParentComponent>(parent)
        .unwrap()
        .children
        .push(ent_entry_with_children);

    ec.as_mut().set_components(ent_entry, (
        GlobalRectComponent::default(),
        LocalRectComponent {
            scale: Vec2::new(Some(UiVal::pd(1.0)).into(), Some(UiVal::px(20.0)).into()),
            ..Default::default()
        },
        ChildComponent { parent: ent_entry_with_children },
        ParentComponent { children: vec![].into() },
        BatchedMeshComponent::default(),
        StandardMaterialComponent {
            material: material_panel.clone(),
        },
        ImageComponent {
            color: Vec4::splat(0.0),
            ..Default::default()
        },
    ));
    ec.get_component_mut::<ParentComponent>(ent_entry_with_children)
        .unwrap()
        .children
        .push(ent_entry);

    ec.as_mut().set_components(ent_entry_name, (
        DebugNameComponent("ent_name".into()),
        GlobalRectComponent::default(),
        LocalRectComponent { ..Default::default() },
        ChildComponent {
            parent: ent_entry,
        },
        BatchedMeshComponent::default(),
        StandardMaterialComponent {
            material: material_text.clone(),
        },
        TextComponent {
            color: Vec4::from_array(parse_color_rgba_f32("#000000ff").unwrap()),
            font: font.clone(),
            font_size: UiVal::px(18.0),
            is_y_inverted: true,
            text: name,
            horizontal_spacing: UiVal::px(0.0),
            vertical_align: VerticalAlign::Middle,
            horizontal_align: HorizontalAlign::Left,
        },
    ));

    let ent_children = ec.create_entity();
    let ent_children_container = ec.create_entity();

    ec.as_mut().set_components(ent_children, (
        DebugNameComponent("ent_children".into()),
        GlobalRectComponent::default(),
        LocalRectComponent {
            scale: Vec2::new(Some(UiVal::pd(1.0)).into(), None.into()),
            ..Default::default()
        },
        ChildComponent { parent: ent_entry_with_children },
    ));
    ec.get_component_mut::<ParentComponent>(ent_entry_with_children)
        .unwrap()
        .children
        .push(ent_children);

    ec.as_mut().set_components(ent_children_container, (
        GlobalRectComponent::default(),
        LocalRectComponent {
            parent_padding_min: Vec2::new(UiVal::px(20.0), UiVal::px(0.0)),
            scale: Vec2::new(Some(UiVal::pd(1.0)).into(), None.into()),
            ..Default::default()
        },
        ChildComponent { parent: ent_children },
        ParentComponent { children: vec![].into() },
        RectChildAlignComponent {
            anchor: Vec2::new(0.0, 0.0),
            direction: UiDirection::Vertical,
            min_gap: UiVal::px(0.0),
            spacing: UiSpacing::Chunk,
            ..Default::default()
        },
    ));

    SpawnedHierarchyEntry {
        ent_entry_with_children,
        ent_entry,
        ent_entry_name,
        ent_children,
        ent_children_container,
    }
}
