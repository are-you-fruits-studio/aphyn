use crate::{
    features::{scene_window::SceneViewImageComponent, ui_interaction::HoverAreaComponent},
    prefabs::WindowComponent,
    *,
};

pub fn init_window_as_scene(mut world: WorldDataMut, ent_window: EntityId) {
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
            SceneViewImageComponent,
        ),
    );
}
