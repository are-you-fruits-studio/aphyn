use aphyn::*;

use crate::{
    features::{
        scroll::ScrollHandleAreaComponent,
        ui_interaction::{ButtonComponent, DragAreaComponent, HoverAreaComponent},
    },
    resources::StandardAssetsResource,
};

pub const WINDOW_HEADER_HEIGHT: f32 = 20.0;
pub const WINDOW_TAB_STRIP_HEIGHT: f32 = 20.0;

const TAB_FONT_SIZE: f32 = 16.0;
const TAB_PADDING: f32 = 6.0;
const TAB_CLOSE_SIZE: f32 = 14.0;

pub const TAB_COLOR_ACTIVE: Vec4<f32> = Vec4::new(0.68, 0.68, 0.68, 1.0);
pub const TAB_COLOR_INACTIVE: Vec4<f32> = Vec4::new(0.45, 0.45, 0.45, 1.0);

#[derive(Component, Debug, Clone)]
pub struct WindowComponent {
    pub kind: FfiString,
    pub content_container: EntityId,
    pub tab: EntityId,
    pub tab_close_button: EntityId,
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

    let tab_width = kind.chars().count() as f32 * TAB_FONT_SIZE * character_ratio + TAB_PADDING * 3.0 + TAB_CLOSE_SIZE;

    let ent_root = ec.create_entity();
    let ent_scroll_handle_area = ec.create_entity();
    let ent_scroll_handle = ec.create_entity();
    let ent_scroll_view = ec.create_entity();
    let ent_scroll_content = ec.create_entity();
    let ent_tab = ec.create_entity();
    let ent_tab_text = ec.create_entity();
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
                color: Vec4::from_array(parse_color_rgba_f32("#757575ff").unwrap()),
                ..Default::default()
            },
            WindowComponent {
                kind: kind.into(),
                content_container: ent_scroll_content,
                tab: ent_tab,
                tab_close_button: ent_tab_close_btn,
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
                scale: Vec2::new(UiVal::px(20.0).into(), UiVal::pd(1.0).into()),
                ..Default::default()
            },
            ChildComponent { parent: ent_root },
            BatchedMeshComponent::default(),
            MaterialComponent {
                material: material_panel.clone(),
            },
            ImageComponent {
                color: Vec4::from_array(parse_color_rgba_f32("#686868ff").unwrap()),
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
                scale: Vec2::new(UiVal::pd(1.0).into(), UiVal::px(100.0).into()),
                ..Default::default()
            },
            ChildComponent {
                parent: ent_scroll_handle_area,
            },
            BatchedMeshComponent::default(),
            MaterialComponent {
                material: material_panel.clone(),
            },
            ImageComponent {
                color: Vec4::from_array(parse_color_rgba_f32("#bebebeff").unwrap()),
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
                parent_padding_max: Vec2::new(UiVal::px(20.0), UiVal::px(0.0)),
                ..Default::default()
            },
            ChildComponent { parent: ent_root },
            ChildrenRectMaskComponent,
            BatchedMeshComponent::default(),
            MaterialComponent {
                material: material_panel.clone(),
            },
            ImageComponent {
                color: Vec4::from_array(parse_color_rgba_f32("#575757ff").unwrap()),
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
            ChildComponent { parent: ent_scroll_view },
            RectChildAlignComponent {
                anchor: Vec2::new(0.0, 0.0),
                direction: UiDirection::Vertical,
                min_gap: UiVal::px(0.0),
                spacing: UiSpacing::Chunk,
                ..Default::default()
            },
            ParentComponent { children: vec![].into() },
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
            ParentComponent {
                children: vec![ent_tab_text, ent_tab_close_btn].into(),
            },
            BatchedMeshComponent::default(),
            MaterialComponent {
                material: material_panel.clone(),
            },
            ImageComponent {
                color: TAB_COLOR_INACTIVE,
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
                parent_padding_max: Vec2::new(UiVal::px(TAB_PADDING * 2.0 + TAB_CLOSE_SIZE), UiVal::px(0.0)),
                ..Default::default()
            },
            ChildComponent { parent: ent_tab },
            BatchedMeshComponent::default(),
            MaterialComponent {
                material: material_text.clone(),
            },
            TextComponent {
                color: Vec4::from_array(parse_color_rgba_f32("#000000ff").unwrap()),
                font: font.clone(),
                font_size: UiVal::px(TAB_FONT_SIZE),
                is_y_inverted: true,
                text: kind.into(),
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
                parent_padding_max: Vec2::new(UiVal::px(TAB_PADDING), UiVal::px(0.0)),
                anchor: Vec2::new(1.0, 0.5),
                pivot: Vec2::new(1.0, 0.5),
                scale: Vec2::splat(UiVal::px(TAB_CLOSE_SIZE).into()),
                ..Default::default()
            },
            ChildComponent { parent: ent_tab },
            ParentComponent {
                children: vec![ent_tab_close_btn_text].into(),
            },
            BatchedMeshComponent::default(),
            MaterialComponent {
                material: material_panel.clone(),
            },
            ImageComponent {
                color: Vec4::from_array(parse_color_rgba_f32("#d04040ff").unwrap()),
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
            ChildComponent {
                parent: ent_tab_close_btn,
            },
            BatchedMeshComponent::default(),
            MaterialComponent {
                material: material_text.clone(),
            },
            TextComponent {
                color: Vec4::from_array(parse_color_rgba_f32("#000000ff").unwrap()),
                font,
                font_size: UiVal::px(TAB_CLOSE_SIZE),
                is_y_inverted: true,
                text: String::from("x").into(),
                horizontal_spacing: UiVal::px(0.0),
                vertical_align: VerticalAlign::Middle,
                horizontal_align: HorizontalAlign::Middle,
            },
        ),
    );

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
                color: Vec4::from_array(parse_color_rgba_f32("#d0d0d0ff").unwrap()),
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
                ChildComponent { parent: ent_root },
                BatchedMeshComponent::default(),
                MaterialComponent {
                    material: material_panel.clone(),
                },
                ImageComponent {
                    color: Vec4::from_array(parse_color_rgba_f32("#61b5fa00").unwrap()),
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
            ChildComponent { parent: ent_root },
            BatchedMeshComponent::default(),
            MaterialComponent {
                material: material_panel.clone(),
            },
            ImageComponent {
                color: Vec4::from_array(parse_color_rgba_f32("#adadadff").unwrap()),
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
            ChildComponent {
                parent: ent_bordered_root,
            },
            DragAreaComponent::default(),
            BatchedMeshComponent::default(),
            MaterialComponent {
                material: material_panel.clone(),
            },
            ImageComponent {
                color: Vec4::from_array(parse_color_rgba_f32("#6b6b6bff").unwrap()),
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
            ChildComponent {
                parent: ent_bordered_root,
            },
            ParentComponent { children: vec![].into() },
            RectChildAlignComponent {
                anchor: Vec2::new(0.0, 0.5),
                direction: UiDirection::Horizontal,
                min_gap: UiVal::px(1.0),
                spacing: UiSpacing::Chunk,
            },
            ChildrenRectMaskComponent,
            BatchedMeshComponent::default(),
            MaterialComponent {
                material: material_panel.clone(),
            },
            ImageComponent {
                color: Vec4::from_array(parse_color_rgba_f32("#929292ff").unwrap()),
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
            ChildComponent {
                parent: ent_bordered_root,
            },
            ParentComponent { children: vec![].into() },
        ),
    );

    ent_root
}
