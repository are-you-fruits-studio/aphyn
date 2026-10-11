use fruits_engine::*;

use crate::{features::{test_window::{HierarchyWindowButtonComponent, InspectorWindowButtonComponent, ProjectWindowButtonComponent, TestWindowButtonComponent}, ui_interaction::ButtonComponent}, resources::StandardAssetsResource};

fn create_header_button<C: 'static>(
    mut ent: EntitiesHolderMut,
    assets_res: &StandardAssetsResource,
    parent: EntityId,
    component: C,
    text: FfiString,
    width: UiVal,
) {
    let ent_root = ent.create_entity();
    let ent_text = ent.create_entity();

    if let Some(parent_c) = ent.get_component_mut::<ParentComponent>(parent) {
        parent_c.children.push(ent_root);
    }

    ent.set_components(ent_root, (
        GlobalRectComponent::default(),
        LocalRectComponent {
            scale: Vec2::new(width.into(), UiVal::pd(1.0).into()),
            ..Default::default()
        },
        ChildComponent {
            parent: parent,
        },
        ParentComponent {
            children: vec![ent_text].into(),
        },
        ButtonComponent::default(),
        BatchedMeshComponent::default(),
        StandardMaterialComponent {
            material: assets_res.material_panel.clone(),
        },
        ImageComponent {
            color: Vec4::new(0.3, 0.3, 0.3, 1.0),
            ..Default::default()
        },
        component,
    ));

    ent.set_components(ent_text, (
        GlobalRectComponent::default(),
        LocalRectComponent::default(),
        ChildComponent {
            parent: ent_root,
        },
        BatchedMeshComponent::default(),
        StandardMaterialComponent {
            material: assets_res.material_text.clone(),
        },
        TextComponent {
            color: Vec4::splat(1.0),
            font: assets_res.font.clone(),
            font_size: UiVal::px(20.0),
            horizontal_align: HorizontalAlign::Middle,
            vertical_align: VerticalAlign::Middle,
            horizontal_spacing: UiVal::px(0.0),
            is_y_inverted: true,
            text: text,
        },
    ));
}

pub fn editor_window(mut world: WorldDataMut) -> EntityId {
    let (res, mut ent, evt) = world.as_tuple_mut();

    let assets_res = res.get::<StandardAssetsResource>().unwrap();

    const HEADER_HEIGHT: f32 = 20.0;

    let ent_root = ent.create_entity();
    let ent_header = ent.create_entity();
    let ent_dock = ent.create_entity();

    ent.set_components(ent_root, (
        GlobalRectComponent::default(),
        LocalRectComponent::default(),
        ParentComponent {
            children: vec![ent_header, ent_dock].into(),
        },
    ));

    ent.set_components(ent_header, (
        GlobalRectComponent::default(),
        LocalRectComponent {
            anchor: Vec2::new(0.5, 0.0),
            pivot: Vec2::new(0.5, 0.0),
            scale: Vec2::new(UiVal::pd(1.0).into(), UiVal::px(HEADER_HEIGHT).into()),
            ..Default::default()
        },
        ChildComponent {
            parent: ent_root,
        },
        ParentComponent::default(),
        RectChildAlignComponent {
            anchor: Vec2::new(0.0, 0.5),
            direction: UiDirection::Horizontal,
            min_gap: UiVal::px(1.0),
            spacing: UiSpacing::Chunk,
        },
        BatchedMeshComponent::default(),
        StandardMaterialComponent {
            material: assets_res.material_panel.clone(),
        },
        ImageComponent {
            color: Vec4::new(0.2, 0.2, 0.2, 1.0),
            ..Default::default()
        },
    ));

    ent.set_components(ent_dock, (
        GlobalRectComponent::default(),
        LocalRectComponent {
            parent_padding_min: Vec2::new(UiVal::px(0.0), UiVal::px(HEADER_HEIGHT)),
            ..Default::default()
        },
        ChildComponent {
            parent: ent_root,
        },
        BatchedMeshComponent::default(),
        StandardMaterialComponent {
            material: assets_res.material_panel.clone(),
        },
        ImageComponent {
            color: Vec4::new(0.1, 0.1, 0.1, 1.0),
            ..Default::default()
        },
    ));

    create_header_button(ent.as_mut(), assets_res, ent_header, TestWindowButtonComponent, "Test".into(), UiVal::px(80.0));
    create_header_button(ent.as_mut(), assets_res, ent_header, ProjectWindowButtonComponent, "Project".into(), UiVal::px(100.0));
    create_header_button(ent.as_mut(), assets_res, ent_header, HierarchyWindowButtonComponent, "Hierarchy".into(), UiVal::px(120.0));
    create_header_button(ent.as_mut(), assets_res, ent_header, InspectorWindowButtonComponent, "Inspector".into(), UiVal::px(120.0));

    ent_root
}
