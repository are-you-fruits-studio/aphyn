use crate::{
    features::{
        game_window::GameViewImageComponent,
        scene_window::SceneViewImageComponent,
        transform_gizmo::{GizmoSpaceButtonComponent, GizmoTool, GizmoToolButtonComponent},
        ui_interaction::HoverAreaComponent,
    },
    prefabs::{WindowComponent, create_header_button},
    *,
};

const TOOLBAR_HEIGHT: f32 = 22.0;
const TOOLBAR_MARGIN: f32 = 4.0;
const TOOLBAR_GAP: f32 = 1.0;

pub fn init_window_as_scene(mut world: WorldDataMut, ent_window: EntityId) {
    let ent_image = init_window_as_view(world.as_mut(), ent_window, SceneViewImageComponent);

    let (res, mut ent, _) = world.as_tuple_mut();
    let assets_res = res.get::<StandardAssetsResource>().unwrap();

    let buttons = [
        ("None", 64.0, Some(GizmoTool::None)),
        ("Move", 64.0, Some(GizmoTool::Move)),
        ("Rotate", 90.0, Some(GizmoTool::Rotate)),
        ("Scale", 76.0, Some(GizmoTool::Scale)),
        ("Global", 90.0, None),
    ];
    let toolbar_width = buttons.iter().map(|(_, width, _)| width).sum::<f32>() + TOOLBAR_GAP * (buttons.len() - 1) as f32;

    let ent_toolbar = ent.create_entity();

    ent.set_components(
        ent_image,
        (ParentComponent {
            children: vec![ent_toolbar].into(),
        },),
    );

    ent.set_components(
        ent_toolbar,
        (
            GlobalRectComponent::default(),
            LocalRectComponent {
                anchor: Vec2::new(0.0, 0.0),
                pivot: Vec2::new(0.0, 0.0),
                offset: Vec2::new(UiVal::px(TOOLBAR_MARGIN), UiVal::px(TOOLBAR_MARGIN)),
                scale: Vec2::new(UiVal::px(toolbar_width).into(), UiVal::px(TOOLBAR_HEIGHT).into()),
                ..Default::default()
            },
            ChildComponent { parent: ent_image },
            ParentComponent::default(),
            RectChildAlignComponent {
                anchor: Vec2::new(0.0, 0.5),
                direction: UiDirection::Horizontal,
                min_gap: UiVal::px(TOOLBAR_GAP),
                spacing: UiSpacing::Chunk,
            },
        ),
    );

    for (text, width, tool) in buttons {
        match tool {
            Some(tool) => create_header_button(
                ent.as_mut(),
                assets_res,
                ent_toolbar,
                GizmoToolButtonComponent { tool },
                text.into(),
                UiVal::px(width),
            ),
            None => create_header_button(
                ent.as_mut(),
                assets_res,
                ent_toolbar,
                GizmoSpaceButtonComponent,
                text.into(),
                UiVal::px(width),
            ),
        }
    }
}

pub fn init_window_as_game(world: WorldDataMut, ent_window: EntityId) {
    init_window_as_view(world, ent_window, GameViewImageComponent);
}

fn init_window_as_view<C: 'static + Component>(mut world: WorldDataMut, ent_window: EntityId, view_image_c: C) -> EntityId {
    let mut ent = world.entities_mut();

    let ent_scroll_content = ent.get_component::<WindowComponent>(ent_window).unwrap().content_container;
    let ent_scroll_view = ent.get_component::<ChildComponent>(ent_scroll_content).unwrap().parent;

    let ent_image = ent.create_entity();

    ent.set_components(
        ent_image,
        (
            GlobalRectComponent::default(),
            LocalRectComponent::default(),
            ChildComponent { parent: ent_scroll_view },
            BatchedMeshComponent::default(),
            MaterialComponent::default(),
            ImageComponent::default(),
            HoverAreaComponent::default(),
            view_image_c,
        ),
    );

    ent_image
}
