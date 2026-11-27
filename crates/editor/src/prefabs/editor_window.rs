use aphyn::*;

use crate::{
    features::{
        play_mode::{PlayButtonComponent, ProjectTitleTextComponent}, test_window::WindowsButtonComponent, ui_interaction::ButtonComponent,
        ui_window::DockComponent,
    },
    resources::StandardAssetsResource,
    theme::{self, PAL},
};

pub(crate) fn create_header_button<C: 'static>(
    mut ent: EntitiesHolderMut,
    assets_res: &StandardAssetsResource,
    parent: EntityId,
    component: C,
    text: FfiString,
    width: UiVal,
) {
    let ent_root = ent.create_entity();
    let ent_text = ent.create_entity();

    ent.set_components(
        ent_root,
        (
            GlobalRectComponent::default(),
            GlobalDisableableComponent::default(),
            LocalRectComponent {
                scale: Vec2::new(width.into(), UiVal::px(theme::CONTROL_H_SM).into()),
                ..Default::default()
            },
            ButtonComponent::default(),
            BatchedMeshComponent::default(),
            MaterialComponent {
                material: assets_res.material_panel.clone(),
            },
            ImageComponent {
                color: PAL.surface_raised,
                ..Default::default()
            },
            component,
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
                material: assets_res.material_text.clone(),
            },
            TextComponent {
                color: PAL.text_1,
                font: assets_res.font.clone(),
                font_size: UiVal::px(theme::FONT),
                horizontal_align: HorizontalAlign::Middle,
                vertical_align: VerticalAlign::Middle,
                horizontal_spacing: UiVal::px(0.0),
                is_y_inverted: true,
                text,
            },
        ),
    );

    set_parent(ent.as_mut(), ent_root, parent);
    set_parent(ent.as_mut(), ent_text, ent_root);
}

pub fn editor_window(mut world: WorldDataMut) -> EntityId {
    let (res, mut ent, _evt) = world.as_tuple_mut();

    let assets_res = res.get::<StandardAssetsResource>().unwrap();

    const HEADER_HEIGHT: f32 = theme::MENUBAR_H;
    const PLAY_BAR_HEIGHT: f32 = theme::TOOLBAR_H;

    let ent_root = ent.create_entity();
    let ent_header = ent.create_entity();
    let ent_play_bar = ent.create_entity();
    let ent_dock = ent.create_entity();
    let ent_title = ent.create_entity();

    ent.set_components(ent_root, (GlobalRectComponent::default(), LocalRectComponent::default()));

    ent.set_components(
        ent_header,
        (
            GlobalRectComponent::default(),
            LocalRectComponent {
                anchor: Vec2::new(0.5, 0.0),
                pivot: Vec2::new(0.5, 0.0),
                scale: Vec2::new(UiVal::pd(1.0).into(), UiVal::px(HEADER_HEIGHT).into()),
                ..Default::default()
            },
            ParentComponent::default(),
            RectChildAlignComponent {
                anchor: Vec2::new(0.0, 0.5),
                direction: UiDirection::Horizontal,
                min_gap: UiVal::px(1.0),
                spacing: UiSpacing::Chunk,
            },
            BatchedMeshComponent::default(),
            MaterialComponent {
                material: assets_res.material_panel.clone(),
            },
            ImageComponent {
                color: PAL.bg_sunken,
                ..Default::default()
            },
        ),
    );

    ent.set_components(
        ent_play_bar,
        (
            GlobalRectComponent::default(),
            LocalRectComponent {
                anchor: Vec2::new(0.5, 0.0),
                pivot: Vec2::new(0.5, 0.0),
                offset: Vec2::new(UiVal::px(0.0), UiVal::px(HEADER_HEIGHT)),
                scale: Vec2::new(UiVal::pd(1.0).into(), UiVal::px(PLAY_BAR_HEIGHT).into()),
                ..Default::default()
            },
            ParentComponent::default(),
            RectChildAlignComponent {
                anchor: Vec2::new(0.5, 0.5),
                direction: UiDirection::Horizontal,
                min_gap: UiVal::px(1.0),
                spacing: UiSpacing::Chunk,
            },
            BatchedMeshComponent::default(),
            MaterialComponent {
                material: assets_res.material_panel.clone(),
            },
            ImageComponent {
                color: PAL.bg_app,
                ..Default::default()
            },
        ),
    );

    ent.set_components(
        ent_dock,
        (
            GlobalRectComponent::default(),
            LocalRectComponent {
                parent_padding_min: Vec2::new(UiVal::px(0.0), UiVal::px(HEADER_HEIGHT + PLAY_BAR_HEIGHT)),
                ..Default::default()
            },
            BatchedMeshComponent::default(),
            MaterialComponent {
                material: assets_res.material_panel.clone(),
            },
            ImageComponent {
                color: PAL.night_950,
                ..Default::default()
            },
            DockComponent,
        ),
    );

    ent.set_components(
        ent_title,
        (
            GlobalRectComponent::default(),
            LocalRectComponent {
                anchor: Vec2::new(1.0, 0.0),
                pivot: Vec2::new(1.0, 0.0),
                offset: Vec2::new(UiVal::px(-12.0), UiVal::px(0.0)),
                scale: Vec2::new(UiVal::pw(0.5).into(), UiVal::px(HEADER_HEIGHT).into()),
                z: -1.0,
                ..Default::default()
            },
            BatchedMeshComponent::default(),
            MaterialComponent {
                material: assets_res.material_text.clone(),
            },
            TextComponent {
                color: PAL.text_3,
                font: assets_res.font.clone(),
                font_size: UiVal::px(theme::FONT),
                horizontal_align: HorizontalAlign::Right,
                vertical_align: VerticalAlign::Middle,
                horizontal_spacing: UiVal::px(0.0),
                is_y_inverted: true,
                text: FfiString::new(),
            },
            ProjectTitleTextComponent,
        ),
    );

    set_parent(ent.as_mut(), ent_header, ent_root);
    set_parent(ent.as_mut(), ent_title, ent_root);
    set_parent(ent.as_mut(), ent_play_bar, ent_root);
    set_parent(ent.as_mut(), ent_dock, ent_root);

    create_header_button(
        ent.as_mut(),
        assets_res,
        ent_header,
        WindowsButtonComponent,
        "Windows".into(),
        UiVal::px(72.0),
    );
    create_header_button(
        ent.as_mut(),
        assets_res,
        ent_play_bar,
        PlayButtonComponent,
        "Play".into(),
        UiVal::px(64.0),
    );

    ent_root
}
