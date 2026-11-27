use aphyn::*;

use crate::{
    features::{
        scroll::ScrollHandleAreaComponent,
        ui_interaction::{ButtonComponent, DragAreaComponent, HoverAreaComponent},
    },
    resources::StandardAssetsResource,
    theme::{self, PAL},
};

pub const WINDOW_HEADER_HEIGHT: f32 = theme::GRIP_H;
pub const WINDOW_TAB_STRIP_HEIGHT: f32 = theme::PANEL_HEADER_H;

const TAB_FONT_SIZE: f32 = theme::FONT;
const TAB_PADDING: f32 = 10.0;
const TAB_CLOSE_SIZE: f32 = 16.0;
const TAB_CLOSE_PADDING: f32 = 4.0;
const GRIP_W: f32 = 28.0;
const GRIP_THICKNESS: f32 = 3.0;

pub fn tab_label(kind: &str) -> &str {
    kind.strip_suffix(" Window").unwrap_or(kind)
}

#[derive(Component, Debug, Clone)]
pub struct WindowComponent {
    pub kind: FfiString,
    pub content_container: EntityId,
    pub tab: EntityId,
    pub tab_text: EntityId,
    pub tab_accent: EntityId,
    pub tab_close_button: EntityId,
    pub tab_close_text: EntityId,
}

#[derive(Component, Debug, Clone)]
pub struct WindowGroupComponent {
    pub panels: FfiVec<EntityId>,
    pub active: u64,
    pub is_docked: bool,
    pub header: EntityId,
    pub tab_strip: EntityId,
    pub content_area: EntityId,
    pub scale_border_left: EntityId,
    pub scale_border_right: EntityId,
    pub scale_border_top: EntityId,
    pub scale_border_bottom: EntityId,
    pub scale_corner_top_left: EntityId,
    pub scale_corner_top_right: EntityId,
    pub scale_corner_bottom_left: EntityId,
    pub scale_corner_bottom_right: EntityId,
}

impl WindowGroupComponent {
    pub fn scale_handles(&self) -> [EntityId; 8] {
        [
            self.scale_border_left,
            self.scale_border_right,
            self.scale_border_top,
            self.scale_border_bottom,
            self.scale_corner_top_left,
            self.scale_corner_top_right,
            self.scale_corner_bottom_left,
            self.scale_corner_bottom_right,
        ]
    }
}

pub fn panel(mut world: WorldDataMut, kind: &str) -> EntityId {
    let (res, mut ec, _evt) = world.as_tuple_mut();

    let assets_res = res.as_ref().get::<StandardAssetsResource>().unwrap();

    let font = assets_res.font.clone();
    let material_panel = assets_res.material_panel.clone();
    let material_text = assets_res.material_text.clone();

    let character_ratio = res
        .as_ref()
        .get::<AssetStorageResource<Font>>()
        .unwrap()
        .get(&font)
        .map(|font| font.character_ratio)
        .unwrap_or(1.0);

    let label = tab_label(kind);
    let tab_width =
        label.chars().count() as f32 * TAB_FONT_SIZE * character_ratio + TAB_PADDING + TAB_CLOSE_PADDING * 2.0 + TAB_CLOSE_SIZE;

    let ent_root = ec.create_entity();
    let ent_scroll_handle_area = ec.create_entity();
    let ent_scroll_handle = ec.create_entity();
    let ent_scroll_view = ec.create_entity();
    let ent_scroll_content = ec.create_entity();
    let ent_tab = ec.create_entity();
    let ent_tab_text = ec.create_entity();
    let ent_tab_accent = ec.create_entity();
    let ent_tab_close_btn = ec.create_entity();
    let ent_tab_close_btn_text = ec.create_entity();

    ec.set_components(
        ent_root,
        (
            GlobalRectComponent::default(),
            GlobalDisableableComponent::default(),
            LocalDisableableComponent::default(),
            LocalRectComponent::default(),
            BatchedMeshComponent::default(),
            MaterialComponent {
                material: material_panel.clone(),
            },
            ImageComponent {
                color: PAL.surface_panel,
                ..Default::default()
            },
            WindowComponent {
                kind: kind.into(),
                content_container: ent_scroll_content,
                tab: ent_tab,
                tab_text: ent_tab_text,
                tab_accent: ent_tab_accent,
                tab_close_button: ent_tab_close_btn,
                tab_close_text: ent_tab_close_btn_text,
            },
        ),
    );

    ec.set_components(
        ent_scroll_handle_area,
        (
            GlobalRectComponent::default(),
            GlobalDisableableComponent::default(),
            LocalRectComponent {
                anchor: Vec2::new(1.0, 0.5),
                pivot: Vec2::new(1.0, 0.5),
                scale: Vec2::new(UiVal::px(theme::SCROLLBAR_W).into(), UiVal::pd(1.0).into()),
                ..Default::default()
            },
            BatchedMeshComponent::default(),
            MaterialComponent {
                material: material_panel.clone(),
            },
            ImageComponent {
                color: PAL.surface_panel,
                ..Default::default()
            },
            ButtonComponent::default(),
            ScrollHandleAreaComponent {
                content: ent_scroll_content,
                handle: ent_scroll_handle,
            },
        ),
    );

    ec.set_components(
        ent_scroll_handle,
        (
            GlobalRectComponent::default(),
            GlobalDisableableComponent::default(),
            LocalRectComponent {
                anchor: Vec2::new(0.5, 0.5),
                pivot: Vec2::new(0.5, 0.5),
                scale: Vec2::new(UiVal::px(theme::SCROLLBAR_HANDLE_W).into(), UiVal::px(100.0).into()),
                ..Default::default()
            },
            BatchedMeshComponent::default(),
            MaterialComponent {
                material: material_panel.clone(),
            },
            ImageComponent {
                color: PAL.night_600,
                ..Default::default()
            },
        ),
    );

    ec.set_components(
        ent_scroll_view,
        (
            GlobalRectComponent::default(),
            GlobalDisableableComponent::default(),
            LocalRectComponent {
                parent_padding_max: Vec2::new(UiVal::px(theme::SCROLLBAR_W), UiVal::px(0.0)),
                ..Default::default()
            },
            ChildrenRectMaskComponent,
            BatchedMeshComponent::default(),
            MaterialComponent {
                material: material_panel.clone(),
            },
            ImageComponent {
                color: PAL.surface_panel,
                ..Default::default()
            },
        ),
    );

    ec.set_components(
        ent_scroll_content,
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

    ec.set_components(
        ent_tab,
        (
            GlobalRectComponent::default(),
            LocalRectComponent {
                scale: Vec2::new(UiVal::px(tab_width).into(), UiVal::pd(1.0).into()),
                ..Default::default()
            },
            BatchedMeshComponent::default(),
            MaterialComponent {
                material: material_panel.clone(),
            },
            ImageComponent {
                color: PAL.bg_sunken,
                ..Default::default()
            },
            ButtonComponent::default(),
            DragAreaComponent::default(),
        ),
    );

    ec.set_components(
        ent_tab_text,
        (
            GlobalRectComponent::default(),
            LocalRectComponent {
                parent_padding_min: Vec2::new(UiVal::px(TAB_PADDING), UiVal::px(0.0)),
                parent_padding_max: Vec2::new(UiVal::px(TAB_CLOSE_PADDING * 2.0 + TAB_CLOSE_SIZE), UiVal::px(0.0)),
                ..Default::default()
            },
            BatchedMeshComponent::default(),
            MaterialComponent {
                material: material_text.clone(),
            },
            TextComponent {
                color: PAL.text_2,
                font: font.clone(),
                font_size: UiVal::px(TAB_FONT_SIZE),
                is_y_inverted: true,
                text: label.into(),
                horizontal_spacing: UiVal::px(0.0),
                vertical_align: VerticalAlign::Middle,
                horizontal_align: HorizontalAlign::Left,
            },
        ),
    );

    ec.set_components(
        ent_tab_close_btn,
        (
            GlobalRectComponent::default(),
            LocalRectComponent {
                parent_padding_max: Vec2::new(UiVal::px(TAB_CLOSE_PADDING), UiVal::px(0.0)),
                anchor: Vec2::new(1.0, 0.5),
                pivot: Vec2::new(1.0, 0.5),
                scale: Vec2::splat(UiVal::px(TAB_CLOSE_SIZE).into()),
                ..Default::default()
            },
            BatchedMeshComponent::default(),
            MaterialComponent {
                material: material_panel.clone(),
            },
            ImageComponent {
                color: PAL.bg_sunken,
                ..Default::default()
            },
            ButtonComponent::default(),
        ),
    );

    ec.set_components(
        ent_tab_close_btn_text,
        (
            GlobalRectComponent::default(),
            LocalRectComponent::default(),
            BatchedMeshComponent::default(),
            MaterialComponent {
                material: material_text.clone(),
            },
            TextComponent {
                color: PAL.text_3,
                font,
                font_size: UiVal::px(theme::FONT),
                is_y_inverted: true,
                text: String::from("x").into(),
                horizontal_spacing: UiVal::px(0.0),
                vertical_align: VerticalAlign::Middle,
                horizontal_align: HorizontalAlign::Middle,
            },
        ),
    );

    ec.set_components(
        ent_tab_accent,
        (
            GlobalRectComponent::default(),
            GlobalDisableableComponent::default(),
            LocalDisableableComponent { is_disabled: true },
            LocalRectComponent {
                anchor: Vec2::new(0.5, 0.0),
                pivot: Vec2::new(0.5, 0.0),
                scale: Vec2::new(UiVal::pd(1.0).into(), UiVal::px(theme::BORDER).into()),
                ..Default::default()
            },
            BatchedMeshComponent::default(),
            MaterialComponent {
                material: material_panel.clone(),
            },
            ImageComponent {
                color: PAL.accent_hover,
                ..Default::default()
            },
        ),
    );

    set_parent(ec.as_mut(), ent_scroll_handle_area, ent_root);
    set_parent(ec.as_mut(), ent_scroll_handle, ent_scroll_handle_area);
    set_parent(ec.as_mut(), ent_scroll_view, ent_root);
    set_parent(ec.as_mut(), ent_scroll_content, ent_scroll_view);
    set_parent(ec.as_mut(), ent_tab_text, ent_tab);
    set_parent(ec.as_mut(), ent_tab_accent, ent_tab);
    set_parent(ec.as_mut(), ent_tab_close_btn, ent_tab);
    set_parent(ec.as_mut(), ent_tab_close_btn_text, ent_tab_close_btn);

    ent_root
}

pub fn window_group(mut world: WorldDataMut) -> EntityId {
    let (res, mut ec, _evt) = world.as_tuple_mut();

    let material_panel = res.as_ref().get::<StandardAssetsResource>().unwrap().material_panel.clone();

    let ent_root = ec.create_entity();
    let ent_scale_border_left = ec.create_entity();
    let ent_scale_border_right = ec.create_entity();
    let ent_scale_border_top = ec.create_entity();
    let ent_scale_border_bottom = ec.create_entity();
    let ent_scale_corner_top_left = ec.create_entity();
    let ent_scale_corner_top_right = ec.create_entity();
    let ent_scale_corner_bottom_left = ec.create_entity();
    let ent_scale_corner_bottom_right = ec.create_entity();
    let ent_bordered_root = ec.create_entity();
    let ent_header = ec.create_entity();
    let ent_tab_strip = ec.create_entity();
    let ent_content_area = ec.create_entity();
    let ent_header_grip = ec.create_entity();

    ec.as_mut().set_components(
        ent_root,
        (
            GlobalRectComponent::default(),
            LocalRectComponent {
                anchor: Vec2::new(0.0, 0.0),
                pivot: Vec2::new(0.0, 0.0),
                scale: Vec2::new(UiVal::px(300.0).into(), UiVal::px(300.0).into()),
                ..Default::default()
            },
            BatchedMeshComponent::default(),
            MaterialComponent {
                material: material_panel.clone(),
            },
            ImageComponent {
                color: PAL.border_default,
                ..Default::default()
            },
            HoverAreaComponent::default(),
            WindowGroupComponent {
                panels: FfiVec::new(),
                active: 0,
                is_docked: false,
                header: ent_header,
                tab_strip: ent_tab_strip,
                content_area: ent_content_area,
                scale_border_left: ent_scale_border_left,
                scale_border_right: ent_scale_border_right,
                scale_border_top: ent_scale_border_top,
                scale_border_bottom: ent_scale_border_bottom,
                scale_corner_top_left: ent_scale_corner_top_left,
                scale_corner_top_right: ent_scale_corner_top_right,
                scale_corner_bottom_left: ent_scale_corner_bottom_left,
                scale_corner_bottom_right: ent_scale_corner_bottom_right,
            },
        ),
    );

    let mut init_scale_border_ent = |entity: EntityId, anchor: Vec2<f32>, scale: Vec2<UiVal>, z: f32| {
        ec.as_mut().set_components(
            entity,
            (
                GlobalRectComponent::default(),
                GlobalDisableableComponent::default(),
                LocalDisableableComponent::default(),
                LocalRectComponent {
                    anchor,
                    pivot: anchor,
                    scale: scale.map(|v| v.into()),
                    z,
                    ..Default::default()
                },
                BatchedMeshComponent::default(),
                MaterialComponent {
                    material: material_panel.clone(),
                },
                ImageComponent {
                    color: PAL.transparent,
                    ..Default::default()
                },
                HoverAreaComponent::default(),
                DragAreaComponent::default(),
            ),
        );
    };

    init_scale_border_ent(
        ent_scale_border_left,
        Vec2::new(0.0, 0.5),
        Vec2::new(UiVal::px(5.0), UiVal::pd(1.0)),
        -300.0,
    );
    init_scale_border_ent(
        ent_scale_border_right,
        Vec2::new(1.0, 0.5),
        Vec2::new(UiVal::px(5.0), UiVal::pd(1.0)),
        -300.0,
    );
    init_scale_border_ent(
        ent_scale_border_top,
        Vec2::new(0.5, 0.0),
        Vec2::new(UiVal::pd(1.0), UiVal::px(5.0)),
        -300.0,
    );
    init_scale_border_ent(
        ent_scale_border_bottom,
        Vec2::new(0.5, 1.0),
        Vec2::new(UiVal::pd(1.0), UiVal::px(5.0)),
        -300.0,
    );

    let corner_scale = Vec2::splat(UiVal::px(8.0));
    init_scale_border_ent(ent_scale_corner_top_left, Vec2::new(0.0, 0.0), corner_scale, -301.0);
    init_scale_border_ent(ent_scale_corner_top_right, Vec2::new(1.0, 0.0), corner_scale, -301.0);
    init_scale_border_ent(ent_scale_corner_bottom_left, Vec2::new(0.0, 1.0), corner_scale, -301.0);
    init_scale_border_ent(ent_scale_corner_bottom_right, Vec2::new(1.0, 1.0), corner_scale, -301.0);

    ec.as_mut().set_components(
        ent_bordered_root,
        (
            GlobalRectComponent::default(),
            LocalRectComponent {
                parent_padding_min: Vec2::splat(UiVal::px(1.0)),
                parent_padding_max: Vec2::splat(UiVal::px(1.0)),
                ..Default::default()
            },
            BatchedMeshComponent::default(),
            MaterialComponent {
                material: material_panel.clone(),
            },
            ImageComponent {
                color: PAL.surface_panel,
                ..Default::default()
            },
        ),
    );

    ec.as_mut().set_components(
        ent_header,
        (
            GlobalRectComponent::default(),
            GlobalDisableableComponent::default(),
            LocalDisableableComponent::default(),
            LocalRectComponent {
                anchor: Vec2::new(0.5, 0.0),
                pivot: Vec2::new(0.5, 0.0),
                scale: Vec2::new(UiVal::pd(1.0).into(), UiVal::px(WINDOW_HEADER_HEIGHT).into()),
                ..Default::default()
            },
            DragAreaComponent::default(),
            BatchedMeshComponent::default(),
            MaterialComponent {
                material: material_panel.clone(),
            },
            ImageComponent {
                color: PAL.surface_raised,
                ..Default::default()
            },
        ),
    );

    ec.as_mut().set_components(
        ent_header_grip,
        (
            GlobalRectComponent::default(),
            GlobalDisableableComponent::default(),
            LocalRectComponent {
                anchor: Vec2::new(0.5, 0.5),
                pivot: Vec2::new(0.5, 0.5),
                scale: Vec2::new(UiVal::px(GRIP_W).into(), UiVal::px(GRIP_THICKNESS).into()),
                ..Default::default()
            },
            BatchedMeshComponent::default(),
            MaterialComponent {
                material: material_panel.clone(),
            },
            ImageComponent {
                color: PAL.night_600,
                ..Default::default()
            },
        ),
    );

    ec.as_mut().set_components(
        ent_tab_strip,
        (
            GlobalRectComponent::default(),
            LocalRectComponent {
                anchor: Vec2::new(0.5, 0.0),
                pivot: Vec2::new(0.5, 0.0),
                offset: Vec2::new(UiVal::px(0.0), UiVal::px(WINDOW_HEADER_HEIGHT)),
                scale: Vec2::new(UiVal::pd(1.0).into(), UiVal::px(WINDOW_TAB_STRIP_HEIGHT).into()),
                ..Default::default()
            },
            ParentComponent::default(),
            RectChildAlignComponent {
                anchor: Vec2::new(0.0, 0.5),
                direction: UiDirection::Horizontal,
                min_gap: UiVal::px(theme::BORDER),
                spacing: UiSpacing::Chunk,
            },
            ChildrenRectMaskComponent,
            BatchedMeshComponent::default(),
            MaterialComponent {
                material: material_panel.clone(),
            },
            ImageComponent {
                color: PAL.bg_sunken,
                ..Default::default()
            },
        ),
    );

    ec.as_mut().set_components(
        ent_content_area,
        (
            GlobalRectComponent::default(),
            LocalRectComponent {
                parent_padding_min: Vec2::new(UiVal::px(0.0), UiVal::px(WINDOW_HEADER_HEIGHT + WINDOW_TAB_STRIP_HEIGHT)),
                ..Default::default()
            },
            ParentComponent::default(),
        ),
    );

    for ent_border in [
        ent_scale_border_left,
        ent_scale_border_right,
        ent_scale_border_top,
        ent_scale_border_bottom,
        ent_scale_corner_top_left,
        ent_scale_corner_top_right,
        ent_scale_corner_bottom_left,
        ent_scale_corner_bottom_right,
        ent_bordered_root,
    ] {
        set_parent(ec.as_mut(), ent_border, ent_root);
    }
    set_parent(ec.as_mut(), ent_header, ent_bordered_root);
    set_parent(ec.as_mut(), ent_header_grip, ent_header);
    set_parent(ec.as_mut(), ent_tab_strip, ent_bordered_root);
    set_parent(ec.as_mut(), ent_content_area, ent_bordered_root);

    ent_root
}
