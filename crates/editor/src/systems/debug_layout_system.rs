use crate::*;

pub fn debug_layout_system(
    mut world: WorldDataMut,
) {
    let aligns = world.as_ref().entities().query_filtered::<EntityId, WithFilter<RectChildAlignComponent>>().iter().collect::<Vec<_>>();

    for align in aligns {
        let children = world.as_mut().entities_mut().get_component_mut::<ParentComponent>(align).map(|p| p.children.as_slice()).unwrap_or(&[]).to_vec();

        let mut children_debug = Vec::new();

        for child in children {
            if let Some(name) = world.as_ref().entities().get_component::<DebugNameComponent>(child).map(|c| c.0.as_str()) {
                children_debug.push(name);
            }
        }

        if children_debug.len() > 1 {
            println!("{:?}" , children_debug);
        }
    }
}
