use crate::*;

pub fn register_feature(mut world: WorldBuilderMut) {
    world.data_mut().resources_mut().insert(GenericDropdownResource::default());

    let mut behavior = world.behavior_mut();
    let mut update = behavior.get_mut(Schedule::Update);

    update.group(SYSTEM_GROUP)
        .insert_child_system(delete_generic_dropdown_system)
        .insert_child_system(spawn_generic_dropdown_system)
        .insert_child_system(select_generic_dropdown_system);

    update.order_system(check_ui_interaction_system)
        .before_system(delete_generic_dropdown_system)
        .before_system(spawn_generic_dropdown_system)
        .before_system(select_generic_dropdown_system);
}

//

#[repr(C)]
#[derive(Debug, Default, Clone)]
pub struct GenericDropdownState {
    pub variants: FfiVec<FfiString>,
    pub ty: GenericDropdownStateTy,
}

#[repr(C)]
#[derive(Debug, Default, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum GenericDropdownStateTy {
    #[default]
    Dismissed,
    Waiting,
    Selected { idx: u64 },
}

#[repr(C)]
#[derive(Resource, Debug, Clone, Default)]
pub struct GenericDropdownResource {
    last_id: u64,
    last_state: GenericDropdownState,
}

impl GenericDropdownResource {
    pub fn request(&mut self, variants: FfiVec<FfiString>) -> u64 {
        self.last_state.variants = variants;
        self.last_state.ty = GenericDropdownStateTy::Waiting;
        self.last_id = self.last_id.wrapping_add(1);
        self.last_id
    }

    pub fn get_state(&self, id: u64) -> &GenericDropdownState {
        if self.last_id != id {
            return const { &GenericDropdownState {
                variants: FfiVec::new(),
                ty: GenericDropdownStateTy::Dismissed,
            } };
        }

        &self.last_state
    }

    pub fn try_dismiss(&mut self, id: u64) -> bool {
        if self.last_id != id {
            return false;
        }

        let GenericDropdownStateTy::Waiting = self.last_state.ty else {
            return false;
        };

        self.last_state.ty = GenericDropdownStateTy::Dismissed;
        return true;
    }

    pub fn try_select(&mut self, id: u64, idx: u64) -> bool {
        if self.last_id != id {
            return false;
        }

        let GenericDropdownStateTy::Waiting = self.last_state.ty else {
            return false;
        };

        if idx >= self.last_state.variants.len() {
            return false;
        }

        self.last_state.ty = GenericDropdownStateTy::Selected { idx };
        return true;
    }
}

#[repr(C)]
#[derive(Component, Debug, Clone)]
pub struct GenericDropdownComponent {
    id: u64,
    variants_container: EntityId,
}

#[repr(C)]
#[derive(Component, Debug, Copy, Clone)]
pub struct GenericDropdownEntryComponent {
    pub dropdown: EntityId,
    pub text: EntityId,
    pub idx: u64,
}

pub fn delete_generic_dropdown_system(mut world: WorldDataMut) {
    let (res, mut ent, evt) = world.as_tuple_mut();
    let generic_dropdown_resource = res.get::<GenericDropdownResource>().unwrap();

    let mut entities_to_destroy = Vec::new();

    if generic_dropdown_resource.last_state.ty != GenericDropdownStateTy::Waiting {
        for ent_dropdown in ent.query_filtered::<EntityId, WithFilter<GenericDropdownComponent>>().iter().collect::<Vec<_>>() {
            entities_to_destroy.push(ent_dropdown);
        }
    } else {
        for (ent_dropdown, dropdown_c) in ent.query::<(EntityId, &GenericDropdownComponent)>().iter() {
            if dropdown_c.id != generic_dropdown_resource.last_id {
                entities_to_destroy.push(ent_dropdown);
            }
        }
    }

    for ent_to_destroy in entities_to_destroy {
        destroy_entity_and_children(ent.as_mut(), ent_to_destroy);
    }
}

pub fn spawn_generic_dropdown_system(mut world: WorldDataMut) {
    let (res, mut ent, evt) = world.as_tuple_mut();

    if !ent.query::<&GenericDropdownComponent>().is_empty() {
        return;
    }

    let standard_assets_resource = res.get::<StandardAssetsResource>().unwrap();
    let generic_dropdown_resource = res.get::<GenericDropdownResource>().unwrap();

    let GenericDropdownStateTy::Waiting = generic_dropdown_resource.last_state.ty else {
        return;
    };

    let ent_dropdown = ent.create_entity();

    ent.set_components(ent_dropdown, (
        GlobalRectComponent::default(),
        LocalRectComponent::default(),
        ParentComponent::default(),
        RectChildAlignComponent {
            ..Default::default()
        },
        GenericDropdownComponent {
            id: generic_dropdown_resource.last_id,
            variants_container: ent_dropdown,
        }
    ));

    for (idx, variant) in generic_dropdown_resource.last_state.variants.iter().enumerate() {
        let ent_dropdown_entry = ent.create_entity();
        let ent_dropdown_entry_text = ent.create_entity();

        ent.get_component_mut::<ParentComponent>(ent_dropdown).unwrap().children.push(ent_dropdown_entry);

        ent.set_components(ent_dropdown_entry, (
            GlobalRectComponent::default(),
            LocalRectComponent::default(),
            ChildComponent {
                parent: ent_dropdown,
            },
            ParentComponent {
                children: vec![ent_dropdown_entry_text].into(),
            },
            ButtonComponent::default(),
            GenericDropdownEntryComponent {
                dropdown: ent_dropdown,
                text: ent_dropdown_entry_text,
                idx: idx as u64,
            }
        ));

        ent.set_components(ent_dropdown_entry_text, (
            GlobalRectComponent::default(),
            LocalRectComponent::default(),
            ChildComponent {
                parent: ent_dropdown_entry,
            },
            BatchedMeshComponent::default(),
            StandardMaterialComponent { 
                material: standard_assets_resource.material_text.clone(),
            },
            TextComponent {
                color: Vec4::splat(1.0),
                font_size: UiVal::px(20.0),
                horizontal_align: HorizontalAlign::Left,
                vertical_align: VerticalAlign::Middle,
                horizontal_spacing: UiVal::px(0.0),
                is_y_inverted: true,
                text: variant.clone(),
                font: standard_assets_resource.font.clone(),
            }
        ));
    }
}

pub fn select_generic_dropdown_system(
    input_res: Res<InputResource>,
    mut generic_dropdown_res: ResMut<GenericDropdownResource>,
    dropdown_q: WorldQuery<&GenericDropdownComponent>,
    dropdown_entry_q: WorldQuery<(&GenericDropdownEntryComponent, &ButtonComponent)>
) {
    for (dropdown_entry_c, button_c) in dropdown_entry_q.iter() {
        if !button_c.was_clicked_this_frame {
            continue;
        }

        let Some(dropdown_c) = dropdown_q.get(dropdown_entry_c.dropdown) else {
            continue;
        };

        generic_dropdown_res.try_select(dropdown_c.id, dropdown_entry_c.idx);
        return;
    }

    if input_res.mouse.is_just_pressed(MouseButton::Left) {
        let id = generic_dropdown_res.last_id;
        generic_dropdown_res.try_dismiss(id);
    }
}