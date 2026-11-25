//! # aphyn_prefab
//!
//! Defines the engine's in-memory prefab and scene formats — reusable templates of entities and
//! their serialized components — together with the helpers that convert an entity's components
//! to and from that format. A [`Prefab`] has a root entity (its first entity); a [`Scene`] has
//! none and is just a set of entities. Spawned entities keep no link to the prefab or scene they
//! came from.
//!
//! # How to use
//!
//! #### Opting a component into prefabs
//!
//! Prefab components are (de)serialized by their serialized name through the world's
//! `SerializersResource` (`aphyn_serialization_ecs`), so a component type only round-trips once a serializer for it is
//! registered there during app init. Without one, the component is recorded with a `Null` payload
//! (and a `MissingSerializer` error is reported) and skipped when the prefab is instantiated:
//!
//! ```ignore
//! use aphyn::*;
//!
//! #[derive(Component, Serializable, Default)]
//! struct Health(f32);
//!
//! fn setup(ctx: AppInitCtx) {
//!     ctx.serializers.register(ctx.types, "my_game::Health", StandardSerializer::<Health>::default());
//! }
//! ```
//!
//! #### Building a prefab in memory
//!
//! Assemble a [`Prefab`] directly — useful for tools and tests that produce prefabs without
//! reading a file. Each entry maps a prefab-local entity id to the list of components that
//! entity carries:
//!
//! ```
//! use aphyn_ffi::{FfiIndexMap, FfiString};
//! use aphyn_prefab::Prefab;
//! use aphyn_serialization::SerializedValue;
//!
//! let mut components = FfiIndexMap::<FfiString, SerializedValue>::default();
//! components.insert("my_crate::Health".into(), SerializedValue::Null);
//!
//! let mut prefab = Prefab::empty();
//! prefab.entities.0.insert(1, components.into());
//!
//! assert_eq!(prefab.entities.0.len(), 1);
//! ```
//!
//! A [`Scene`] is built the same way (`Scene::empty()`, then fill `scene.entities`).
//!
//! #### Recording and restoring an entity's components
//!
//! [`serialize_components`] captures every component of an entity as a map from component id
//! to serialized payload, and [`deserialize_entity_components`] adds them to an entity. Both take a registry-backed
//! [`SerializerCtx`]; loading prefab and scene files, instantiating them, and recording
//! hierarchies or whole worlds live in `aphyn_asset_loading`:
//!
//! ```ignore
//! use aphyn::*;
//!
//! let serializers = res.get::<SerializersResource>().unwrap();
//! let mut on_err = |err| println!("{err}");
//! let context = ContextData::new();
//! let mut ctx = serializers.to_ctx(&context, res.types(), &mut on_err);
//!
//! let components = serialize_components(source, ctx.as_mut(), ent.as_ref());
//! deserialize_entity_components(&components, target, ctx, ent.as_mut());
//! ```
//!
//! # How to maintain
//!
//! #### Data model
//!
//! [`Prefab`] and [`Scene`] each hold only [`SerializedEntities`] — an `FfiIndexMap` from a
//! local entity id (`u64`) to [`SerializedEntityComponents`], an `FfiIndexMap` from component
//! id to the component's [`SerializedValue`] payload. All of these are `#[repr(C)]`. The ids are
//! local to the prefab or scene: instantiation creates one real entity per id and resolves entity
//! references inside component data through those ids. Both wrappers implement [`Serializable`]
//! by delegating to their map, and `SerializedValue` is serializable as-is, so the component
//! payloads pass through (de)serialization verbatim. The two types differ only in meaning: a
//! prefab's first entity is its root, a scene has no root. Keep their layouts in sync.
//!
//! Neither stores the assets its components reference. Asset handles stay in the payloads as
//! asset keys and are resolved against the world's asset storages on instantiation (see
//! `aphyn_asset_loading`); precaching them is a `todo`.
//!
//! #### The component id is the serialized name
//!
//! [`serialize_components`] walks the entity's components with `get_all_components`, stores
//! each one's serialized name (the name its serializer was registered with, found by the
//! component's `AnyTypeId`) as the `component_id`, serializes it with
//! [`SerializerCtx::serialize_any`], and sorts the result by `component_id` so the output is
//! stable; a component without a serializer is reported and left out. [`deserialize_component`]
//! goes the other way: it resolves the name to a type id, and if the entity already has that
//! component it deserializes the payload into it in place at the given path; otherwise
//! [`SerializerCtx::deserialize_default_any_by_id`] looks the serializer up,
//! creates the component from the serializer's default, fills it from the payload, and it is
//! attached with `add_component_any`. It returns `false` if no serializer matches or the
//! component cannot be added; [`deserialize_entity_components`] logs such components and
//! moves on.
//!
//! #### Where the rest of the pipeline lives
//!
//! This crate only owns the format and the per-component conversion. Reading prefab and scene
//! files, loading their dependencies, instantiating them into the world, and recording entities
//! into them live in `aphyn_asset_loading`, and the `AssetStorageResource<Prefab>` and
//! `AssetStorageResource<Scene>` are inserted there by `add_asset_module_to`.
//! [`serialize_prefab_single_entity`] and [`deserialize_entity_components`] are marked `todo`:
//! the former always stores the entity under id `0`.

use aphyn_ecs::*;
use aphyn_ffi::{FfiIndexMap, FfiString};
use aphyn_serialization::*;

#[repr(C)]
#[derive(Default, Debug, Clone)]
pub struct SerializedEntityComponents(pub FfiIndexMap<FfiString, SerializedValue>);

impl<S: Copy + SerializerCtxState<FfiString> + SerializerCtxState<SerializedValue>> Serializable<S> for SerializedEntityComponents {
    fn serialize(&self, ctx: SerializerCtx<S>, path: &str) -> SerializedValue {
        self.0.serialize(ctx, path)
    }

    fn deserialize(&mut self, ctx: SerializerCtx<S>, path: &str, serialized: &SerializedValue) {
        self.0.deserialize(ctx, path, serialized)
    }
}

impl From<FfiIndexMap<FfiString, SerializedValue>> for SerializedEntityComponents {
    fn from(value: FfiIndexMap<FfiString, SerializedValue>) -> Self {
        Self(value)
    }
}

#[repr(C)]
#[derive(Default, Debug, Clone)]
pub struct SerializedEntities(pub FfiIndexMap<u64, SerializedEntityComponents>);

impl<S: Copy + SerializerCtxState<u64> + SerializerCtxState<SerializedEntityComponents>> Serializable<S> for SerializedEntities {
    fn serialize(&self, ctx: SerializerCtx<S>, path: &str) -> SerializedValue {
        self.0.serialize(ctx, path)
    }

    fn deserialize(&mut self, ctx: SerializerCtx<S>, path: &str, serialized: &SerializedValue) {
        self.0.deserialize(ctx, path, serialized)
    }
}

impl From<FfiIndexMap<u64, SerializedEntityComponents>> for SerializedEntities {
    fn from(value: FfiIndexMap<u64, SerializedEntityComponents>) -> Self {
        Self(value)
    }
}

#[repr(C)]
#[derive(Debug, Clone)]
pub struct Prefab {
    pub entities: SerializedEntities,
    // todo: precache dependencies (map<str, AssetHandle>)
}

impl Prefab {
    pub fn empty() -> Self {
        Self {
            entities: Default::default(),
        }
    }
}

#[repr(C)]
#[derive(Debug, Clone)]
pub struct Scene {
    pub entities: SerializedEntities,
    // todo: precache dependencies (map<str, AssetHandle>)
}

impl Scene {
    pub fn empty() -> Self {
        Self {
            entities: Default::default(),
        }
    }
}

// todo
pub fn serialize_prefab_single_entity(
    entity: EntityId,
    serializer_ctx: SerializerCtx<TransSerializerCtxState>,
    entities: EntitiesHolderRef,
) -> Prefab {
    Prefab {
        entities: SerializedEntities([
            (0, SerializedEntityComponents(serialize_components(entity, serializer_ctx, entities)))
        ].into_iter().collect()),
    }
}

// todo
pub fn deserialize_entity_components(
    components: &FfiIndexMap<FfiString, SerializedValue>,
    entity: EntityId,
    mut serializer_ctx: SerializerCtx<TransSerializerCtxState>,
    mut entities: EntitiesHolderMut,
) {
    for (component_id, component_value) in components {
        let was_deserialized = deserialize_component(
            entities.as_mut(),
            entity,
            component_id.as_str(),
            serializer_ctx.as_mut(),
            "",
            component_value,
        );

        if !was_deserialized {
            println!("failed to deserialize component: {}", component_id);
        }
    }
}

pub fn serialize_components(
    entity: EntityId,
    mut serializer_ctx: SerializerCtx<TransSerializerCtxState>,
    entities: EntitiesHolderRef,
) -> FfiIndexMap<FfiString, SerializedValue> {
    let mut components: Vec<(FfiString, SerializedValue)> = Vec::new();

    let registry = serializer_ctx.registry();

    entities.get_all_components(entity, |component| match registry.name_of(component.type_id()) {
        Some(name) => components.push((name.into(), serializer_ctx.serialize_any(component, ""))),
        None => serializer_ctx.report_err(SerializationError::MissingSerializer {
            type_name: component.type_info().short().name().into(),
        }),
    });

    // todo: implement sorting directly on FfiIndexMap
    components.sort_by(|l, r| l.0.cmp(&r.0));
    components.into_iter().collect()
}

pub fn deserialize_component(
    mut entities: EntitiesHolderMut,
    entity: EntityId,
    name: &str,
    mut serializer_ctx: SerializerCtx<TransSerializerCtxState>,
    path: &str,
    serialized: &SerializedValue,
) -> bool {
    let Some(id) = serializer_ctx.registry().type_id_of_name(name) else {
        serializer_ctx.report_err(SerializationError::MissingSerializer { type_name: name.into() });
        return false;
    };

    let mut component = None;

    entities.get_all_components_mut(entity, |entity_component| {
        if component.is_some() || entity_component.type_id() != id {
            return;
        }

        component = Some(entity_component);
    });

    if let Some(c) = component {
        serializer_ctx.deserialize_any(c, path, serialized);
        return true;
    }

    let Some(component) = serializer_ctx.deserialize_default_any_by_id(id, path, serialized) else {
        return false;
    };

    entities.add_component_any(entity, component).is_ok()
}
