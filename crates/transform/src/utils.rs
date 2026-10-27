use std::collections::VecDeque;

use aphyn_ecs::{EntitiesHolderMut, EntityId, QueryFilter, WorldQuery};

use crate::{ChildComponent, ParentComponent};

pub fn hierarchy_iter_breadth_first_parent_to_child(
    q: &WorldQuery<(EntityId, Option<&ChildComponent>, Option<&ParentComponent>)>,
    mut f: impl FnMut(EntityId, EntityId),
) {
    let mut ents_to_calc = q
        .iter()
        .filter_map(|(e, c, _p)| {
            let Some(child_component) = c else {
                return Some((e, EntityId::EMPTY));
            };

            if q.get(child_component.parent).is_none() {
                return Some((e, EntityId::EMPTY));
            }

            None
        })
        .collect::<VecDeque<_>>();

    while let Some((entity, parent)) = ents_to_calc.pop_front() {
        let entity_parent_c = q.get(entity).unwrap().2;

        if let Some(entity_parent_c) = entity_parent_c {
            for &child in entity_parent_c.children.iter() {
                if child == entity {
                    continue;
                }

                let Some(child_child_c) = q.get(child).unwrap().1 else {
                    continue;
                };

                if child_child_c.parent != entity {
                    continue;
                }

                ents_to_calc.push_back((child, entity));
            }
        };

        f(entity, parent);
    }
}

/// f(optional parent, children, cache) -> cache
pub fn hierarchy_iter_depth_first_parent_to_child<R, F: QueryFilter>(
    q: &WorldQuery<(EntityId, Option<&ChildComponent>, Option<&ParentComponent>), F>,
    root: Option<EntityId>,
    mut f: impl FnMut(EntityId, &[EntityId], Option<&R>) -> Option<R>,
) {
    hierarchy_iter_depth_first(q, root, move |parent, entity, children, cache, is_moving_to_root| {
        if is_moving_to_root {
            return None;
        }

        let temp_cache;
        let mut cache = cache;

        if parent == EntityId::EMPTY {
            // entity is root. Entity as child
            temp_cache = f(EntityId::EMPTY, &[entity], None);
            cache = temp_cache.as_ref();
        }

        f(entity, children, cache)
    })
}

/// f(optional parent, children)
pub fn hierarchy_iter_depth_first_child_to_parent<R, F: QueryFilter>(
    q: &WorldQuery<(EntityId, Option<&ChildComponent>, Option<&ParentComponent>), F>,
    mut f: impl FnMut(EntityId, &[EntityId]) -> R,
) {
    hierarchy_iter_depth_first::<(), F>(q, None, move |parent, entity, children, _, is_moving_to_root| {
        if !is_moving_to_root {
            return None;
        }

        f(entity, children);

        if parent == EntityId::EMPTY {
            // entity is root. Entity as child
            f(EntityId::EMPTY, &[entity]);
        }

        None
    })
}

/// f(optional parent, entity, children, cache, is_moving_to_root) -> entity cache.
/// Called for every entity twice: moving down (cache is the parent's) and moving to root (cache is the entity's, result ignored).
pub fn hierarchy_iter_depth_first<R, F: QueryFilter>(
    q: &WorldQuery<(EntityId, Option<&ChildComponent>, Option<&ParentComponent>), F>,
    root: Option<EntityId>,
    mut f: impl FnMut(EntityId, EntityId, &[EntityId], Option<&R>, bool) -> Option<R>,
) {
    let roots = match root {
        Some(root) => q.get(root).map(|_| root).into_iter().collect::<Vec<_>>(),
        None => q
            .iter()
            .filter_map(|(e, c, _p)| {
                let Some(child_component) = c else {
                    return Some(e);
                };

                if q.get(child_component.parent).is_none() {
                    return Some(e);
                }

                None
            })
            .collect::<Vec<_>>(),
    };

    let get_children = |ent| -> &[EntityId] {
        let Some(p) = q.get(ent).map(|t| t.2).flatten() else {
            return &[];
        };
        p.children.as_slice()
    };

    let mut stack = VecDeque::<(EntityId, &[EntityId], Option<R>, usize)>::new();

    for root in roots {
        let mut entity = root;
        let mut entity_children = get_children(entity);
        let mut entity_cache = f(EntityId::EMPTY, entity, entity_children, None, false);
        let mut child_idx_to_check = 0;

        loop {
            if let Some(&child_entity) = entity_children.get(child_idx_to_check) {
                let child_entity_children = get_children(child_entity);
                let child_entity_cache = f(entity, child_entity, child_entity_children, entity_cache.as_ref(), false);
                stack.push_back((entity, entity_children, entity_cache, child_idx_to_check + 1));

                entity = child_entity;
                entity_cache = child_entity_cache;
                entity_children = child_entity_children;
                child_idx_to_check = 0;
                continue;
            }

            let parent = stack.back().map(|t| t.0).unwrap_or(EntityId::EMPTY);
            f(parent, entity, entity_children, entity_cache.as_ref(), true);

            let Some((parent_ent, parent_ent_children, parent_ent_cache, parent_idx_to_check)) = stack.pop_back() else {
                break;
            };

            entity = parent_ent;
            entity_cache = parent_ent_cache;
            entity_children = parent_ent_children;
            child_idx_to_check = parent_idx_to_check;
        }
    }
}

pub fn destroy_entity_and_children(mut ec: EntitiesHolderMut, ent: EntityId) {
    destroy_entity_children(ec.as_mut(), ent);

    ec.destroy_entity(ent);
}

pub fn destroy_entity_children(mut ec: EntitiesHolderMut, ent: EntityId) {
    let mut check_buffer = Vec::new();

    if let Some(parent) = ec.as_mut().get_component::<ParentComponent>(ent) {
        check_buffer.extend(parent.children.iter());
    };

    while let Some(ent) = check_buffer.pop() {
        if let Some(parent) = ec.as_mut().get_component::<ParentComponent>(ent) {
            check_buffer.extend(parent.children.iter());
        };

        ec.destroy_entity(ent);
    }
}
