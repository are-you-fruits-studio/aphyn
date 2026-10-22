use crate::{
    features::{game_window::GameViewImageComponent, scene_window::SceneViewImageComponent, ui_interaction::HoverAreaComponent},
    prefabs::WindowComponent,
    *,
};

pub fn init_window_as_scene(world: WorldDataMut, ent_window: EntityId) {
    init_window_as_view(world, ent_window, SceneViewImageComponent);
}

pub fn init_window_as_game(world: WorldDataMut, ent_window: EntityId) {
    init_window_as_view(world, ent_window, GameViewImageComponent);
}

fn init_window_as_view<C: 'static + Component>(mut world: WorldDataMut, ent_window: EntityId, view_image_c: C) {
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
            StandardMaterialComponent::default(),
            ImageComponent::default(),
            HoverAreaComponent::default(),
            view_image_c,
        ),
    );
}
