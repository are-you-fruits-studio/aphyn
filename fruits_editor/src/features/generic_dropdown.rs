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

const DROPDOWN_WIDTH: f32 = 200.0;
const DROPDOWN_ENTRY_HEIGHT: f32 = 20.0;

#[repr(C)]
#[derive(Debug, Default, Clone)]
pub struct GenericDropdownState {
    pub variants: FfiVec<FfiString>,
    pub position: Vec2<f32>,
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
    pub fn request(&mut self, variants: FfiVec<FfiString>, position: Vec2<f32>) -> u64 {
        self.last_state.variants = variants;
        self.last_state.position = position;
        self.last_state.ty = GenericDropdownStateTy::Waiting;
        self.last_id = self.last_id.wrapping_add(1);
        self.last_id
    }

    pub fn get_state(&self, id: u64) -> &GenericDropdownState {
        if self.last_id != id {
            return const { &GenericDropdownState {
                variants: FfiVec::new(),
                position: Vec2::new(0.0, 0.0),
                ty: GenericDropdownStateTy::Dismissed,
            } };
        }

        &self.last_state
    }

    pub fn take_selected(&self, id: &mut u64) -> Option<u64> {
        let GenericDropdownStateTy::Selected { idx } = self.get_state(*id).ty else {
            return None;
        };

        *id = 0;
        Some(idx)
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

    let window_size = Vec2::from_array(res.get::<RenderApiResource>().unwrap().size().map(|v| v as f32));
    let dropdown_size = Vec2::new(
        DROPDOWN_WIDTH,
        DROPDOWN_ENTRY_HEIGHT * generic_dropdown_resource.last_state.variants.len() as f32,
    );
    let dropdown_pos = fit_dropdown_into_window(generic_dropdown_resource.last_state.position, dropdown_size, window_size);

    let ent_dropdown = ent.create_entity();

    ent.set_components(ent_dropdown, (
        GlobalRectComponent::default(),
        LocalRectComponent {
            anchor: Vec2::new(0.0, 0.0),
            pivot: Vec2::new(0.0, 0.0),
            offset: dropdown_pos.map(UiVal::px),
            scale: Vec2::new(Some(UiVal::px(DROPDOWN_WIDTH)).into(), None.into()),
            z: -500.0,
            ..Default::default()
        },
        ParentComponent::default(),
        RectChildAlignComponent {
            anchor: Vec2::new(0.0, 0.0),
            direction: UiDirection::Vertical,
            min_gap: UiVal::px(0.0),
            spacing: UiSpacing::Chunk,
        },
        BatchedMeshComponent::default(),
        StandardMaterialComponent {
            material: standard_assets_resource.material_panel.clone(),
        },
        ImageComponent {
            color: Vec4::from_array(rgba_f32_array!("#272727ff")),
            is_y_inverted: true,
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
            LocalRectComponent {
                scale: Vec2::new(Some(UiVal::pd(1.0)).into(), Some(UiVal::px(DROPDOWN_ENTRY_HEIGHT)).into()),
                ..Default::default()
            },
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

// todo: handle dropdowns bigger than the window
fn fit_dropdown_into_window(cursor: Vec2<f32>, size: Vec2<f32>, window_size: Vec2<f32>) -> Vec2<f32> {
    Vec2::from_fn(|i| {
        let pos = match cursor[i] + size[i] <= window_size[i] {
            true => cursor[i],
            false => cursor[i] - size[i],
        };
        pos.clamp(0.0, (window_size[i] - size[i]).max(0.0))
    })
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

    if input_res.mouse.is_just_pressed(MouseButton::Left) || input_res.mouse.is_just_pressed(MouseButton::Right) {
        let id = generic_dropdown_res.last_id;
        generic_dropdown_res.try_dismiss(id);
    }
}