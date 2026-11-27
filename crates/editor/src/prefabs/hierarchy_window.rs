use crate::{
    components::{HierarchyWindowContentComponent, HierarchyWindowEmptyAreaComponent},
    prefabs::WindowComponent,
    theme::{self, PAL},
    *,
};

pub fn init_window_as_hierarchy(mut world: WorldDataMut, ent_window: EntityId) {
    let (res, mut ent, _evt) = world.as_tuple_mut();
    let assets = res.get::<StandardAssetsResource>().unwrap();

    let ent_scroll_content = ent.get_component::<WindowComponent>(ent_window).unwrap().content_container;
    let ent_scroll_view = ent.get_component::<ChildComponent>(ent_scroll_content).unwrap().parent();

    ent.set_components(
        ent_scroll_view,
        (ButtonComponent::default(), HierarchyWindowEmptyAreaComponent),
    );

    // in front of the scroll view, so entries win the ui raycast over the empty area button
    ent.get_component_mut::<LocalRectComponent>(ent_scroll_content).unwrap().z = -1.0;

    let (ent_source_text, ent_badge_text) = spawn_source_row(ent.as_mut(), assets, ent_scroll_content);

    let ent_entries_container = ent.create_entity();

    ent.set_components(
        ent_entries_container,
        (
            GlobalRectComponent::default(),
            GlobalDisableableComponent::default(),
            LocalRectComponent {
                anchor: Vec2::new(0.0, 0.0),
                pivot: Vec2::new(0.0, 0.0),
                scale: Vec2::new(Some(UiVal::pd(1.0)).into(), None.into()),
                ..Default::default()
            },
            RectChildAlignComponent {
                anchor: Vec2::new(0.0, 0.0),
                direction: UiDirection::Vertical,
                min_gap: UiVal::px(0.0),
                spacing: UiSpacing::Chunk,
                ..Default::default()
            },
            ParentComponent::default(),
        ),
    );
    set_parent(ent.as_mut(), ent_entries_container, ent_scroll_content);

    ent.set_component(
        ent_scroll_content,
        HierarchyWindowContentComponent {
            entries_container: ent_entries_container,
            source_text: ent_source_text,
            badge_text: ent_badge_text,
        },
    );
}

// a row above the entries: the opened asset on the left, its kind (or "play") on the right
fn spawn_source_row(mut ent: EntitiesHolderMut, assets: &StandardAssetsResource, ent_parent: EntityId) -> (EntityId, EntityId) {
    let ent_row = ent.create_entity();
    let ent_separator = ent.create_entity();
    let ent_source = ent.create_entity();
    let ent_badge = ent.create_entity();

    let panel = |color: Vec4<f32>| {
        (
            BatchedMeshComponent::default(),
            MaterialComponent {
                material: assets.material_panel.clone(),
            },
            ImageComponent {
                color,
                ..Default::default()
            },
        )
    };
    let text = |horizontal_align: HorizontalAlign, color: Vec4<f32>| {
        (
            BatchedMeshComponent::default(),
            MaterialComponent {
                material: assets.material_text.clone(),
            },
            TextComponent {
                color,
                font: assets.font.clone(),
                font_size: UiVal::px(theme::FONT),
                is_y_inverted: true,
                text: FfiString::new(),
                horizontal_spacing: UiVal::px(0.0),
                vertical_align: VerticalAlign::Middle,
                horizontal_align,
            },
        )
    };
    let padded = LocalRectComponent {
        parent_padding_min: Vec2::new(UiVal::px(theme::PAD_X), UiVal::px(0.0)),
        parent_padding_max: Vec2::new(UiVal::px(theme::PAD_X), UiVal::px(0.0)),
        ..Default::default()
    };

    ent.set_components(
        ent_row,
        (
            GlobalRectComponent::default(),
            GlobalDisableableComponent::default(),
            LocalRectComponent {
                scale: Vec2::new(UiVal::pd(1.0).into(), UiVal::px(theme::CONTROL_H).into()),
                ..Default::default()
            },
        ),
    );
    ent.set_components(ent_row, panel(PAL.surface_panel));
    ent.set_components(
        ent_separator,
        (
            GlobalRectComponent::default(),
            GlobalDisableableComponent::default(),
            LocalRectComponent {
                scale: Vec2::new(UiVal::pd(1.0).into(), UiVal::px(theme::BORDER).into()),
                ..Default::default()
            },
        ),
    );
    ent.set_components(ent_separator, panel(PAL.border_subtle));
    for (e, align, color) in [
        (ent_source, HorizontalAlign::Left, PAL.text_3),
        (ent_badge, HorizontalAlign::Right, PAL.text_3),
    ] {
        ent.set_components(e, (GlobalRectComponent::default(), GlobalDisableableComponent::default(), padded));
        ent.set_components(e, text(align, color));
        set_parent(ent.as_mut(), e, ent_row);
    }

    set_parent(ent.as_mut(), ent_row, ent_parent);
    set_parent(ent.as_mut(), ent_separator, ent_parent);

    (ent_source, ent_badge)
}
