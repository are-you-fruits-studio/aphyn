use std::collections::VecDeque;

use aphyn_ecs::{EntitiesHolderMut, EntityId, QueryFilter, WorldQuery};

use crate::{ChildComponent, GlobalTransform, LocalTransform, ParentComponent};

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

            if q.get(child_component.parent()).is_none() {
                return Some((e, EntityId::EMPTY));
            }

            None
        })
        .collect::<VecDeque<_>>();

    while let Some((entity, parent)) = ents_to_calc.pop_front() {
        let entity_parent_c = q.get(entity).unwrap().2;

        if let Some(entity_parent_c) = entity_parent_c {
            for &child in entity_parent_c.children() {
                if child == entity {
                    continue;
                }

                let Some(child_child_c) = q.get(child).unwrap().1 else {
                    continue;
                };

                if child_child_c.parent() != entity {
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

                if q.get(child_component.parent()).is_none() {
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
        p.children()
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

pub fn set_parent(ec: EntitiesHolderMut, child: EntityId, parent: EntityId) -> bool {
    set_parent_at(ec, child, parent, usize::MAX)
}

pub fn set_parent_at(mut ec: EntitiesHolderMut, child: EntityId, parent: EntityId, index: usize) -> bool {
    if child == parent || !ec.contains_entity(child) || !ec.contains_entity(parent) || is_ancestor_of(ec.as_mut(), child, parent) {
        return false;
    }

    if ec
        .get_component::<ChildComponent>(child)
        .is_some_and(|c| c.parent() == parent)
        && let Some(parent_c) = ec.get_component::<ParentComponent>(parent)
        && parent_c
            .children()
            .get(index.min(parent_c.children().len().saturating_sub(1)))
            == Some(&child)
    {
        return true;
    }

    unlink_from_parent(ec.as_mut(), child);

    match ec.get_component_mut::<ChildComponent>(child) {
        Some(child_c) => *ChildComponent::parent_mut(child_c) = parent,
        None => _ = ec.set_component(child, ChildComponent::new_unchecked(parent)),
    }

    if ec.get_component::<ParentComponent>(parent).is_none() {
        ec.set_component(parent, ParentComponent::default());
    }

    let children = ParentComponent::children_mut(ec.get_component_mut::<ParentComponent>(parent).unwrap());
    let index = index.min(children.len() as usize);
    children.push(child);
    children.as_mut_slice()[index..].rotate_right(1);

    true
}

pub fn remove_parent(mut ec: EntitiesHolderMut, child: EntityId) -> bool {
    unlink_from_parent(ec.as_mut(), child);
    ec.remove_component::<ChildComponent>(child).is_some()
}

fn unlink_from_parent(mut ec: EntitiesHolderMut, child: EntityId) {
    let Some(parent) = ec.get_component::<ChildComponent>(child).map(|c| c.parent()) else {
        return;
    };
    let Some(parent_c) = ec.get_component_mut::<ParentComponent>(parent) else {
        return;
    };
    let children = ParentComponent::children_mut(parent_c);
    if let Some(idx) = children.as_slice().iter().position(|&e| e == child) {
        children.remove(idx as u64);
    }
}

fn is_ancestor_of(ec: EntitiesHolderMut, ancestor: EntityId, entity: EntityId) -> bool {
    let mut current = entity;
    // bounded so a corrupted (cyclic) hierarchy cannot hang
    for _ in 0..=ec.entities_count() {
        let Some(child_c) = ec.get_component::<ChildComponent>(current) else {
            return false;
        };
        current = child_c.parent();
        if current == ancestor {
            return true;
        }
    }
    false
}

// ignores StaticComponent: meant for computing static entities' stored values by hand
pub fn recalculate_global_transform(mut ec: EntitiesHolderMut, ent: EntityId) {
    let parent_global = ec
        .get_component::<ChildComponent>(ent)
        .and_then(|child_c| ec.get_component::<GlobalTransform>(child_c.parent()))
        .copied()
        .unwrap_or(GlobalTransform::IDENTITY);

    let mut stack = vec![(ent, parent_global)];
    while let Some((ent, parent_global)) = stack.pop() {
        let global = match ec.get_component::<LocalTransform>(ent) {
            Some(local) => {
                let global = parent_global.calculate_child(local);
                match ec.get_component_mut::<GlobalTransform>(ent) {
                    Some(stored) => *stored = global,
                    None => _ = ec.set_component(ent, global),
                }
                global
            }
            None => ec
                .get_component::<GlobalTransform>(ent)
                .copied()
                .unwrap_or(GlobalTransform::IDENTITY),
        };

        if let Some(parent_c) = ec.get_component::<ParentComponent>(ent) {
            stack.extend(parent_c.children().iter().map(|&child| (child, global)));
        }
    }
}

pub fn destroy_entity_and_children(mut ec: EntitiesHolderMut, ent: EntityId) {
    unlink_from_parent(ec.as_mut(), ent);
    destroy_entity_children(ec.as_mut(), ent);

    ec.destroy_entity(ent);
}

pub fn destroy_entity_children(mut ec: EntitiesHolderMut, ent: EntityId) {
    let mut check_buffer = Vec::new();

    if let Some(parent) = ec.as_mut().get_component_mut::<ParentComponent>(ent) {
        let children = ParentComponent::children_mut(parent);
        check_buffer.extend(children.as_slice().iter().copied());
        children.clear();
    };

    while let Some(ent) = check_buffer.pop() {
        if let Some(parent) = ec.as_mut().get_component::<ParentComponent>(ent) {
            check_buffer.extend(parent.children().iter().copied());
        };

        ec.destroy_entity(ent);
    }
}
