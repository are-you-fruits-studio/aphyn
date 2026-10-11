//! # fruits_prefab
//!
//! Defines the engine's in-memory prefab format — a reusable template of entities, their
//! serialized components, and the assets those components reference — together with the
//! helpers that convert an entity's components to and from that format.
//!
//! # How to use
//!
//! #### Opting a component into prefabs
//!
//! Prefab components are (de)serialized by type name through the world's
//! [`SerializersResource`], so a component type only round-trips once a serializer for it is
//! registered there. Without one, the component is recorded with a `Null` payload (and a
//! `MissingSerializer` error is reported) and skipped when the prefab is instantiated:
//!
//! ```ignore
//! use fruits_engine::*;
//!
//! #[derive(Component, Serializable, Default)]
//! struct Health(f32);
//!
//! fn register_serializers(mut world: WorldBuilderMut) {
//!     let mut data = world.data_mut();
//!     let mut res = data.resources_mut();
//!     let serializers = res.get_mut::<SerializersResource>().unwrap();
//!
//!     serializers.register(StandardSerializer::<Health>::default());
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
//! use fruits_prefab::{Prefab, PrefabComponent};
//! use fruits_serialization::SerializedValue;
//!
//! let mut prefab = Prefab::empty();
//! prefab.entities.0.insert(1, vec![
//!     PrefabComponent {
//!         component_id: "my_crate::Health".into(),
//!         data: SerializedValue::Null,
//!     },
//! ].into());
//!
//! assert_eq!(prefab.entities.0.len(), 1);
//! ```
//!
//! #### Recording and restoring an entity's components
//!
//! [`serialize_components`] captures every component of an entity as [`PrefabComponent`]s,
//! and [`deserialize_prefab_components`] adds them to an entity. Both take a registry-backed
//! [`SerializerCtx`]; loading prefab files, instantiating whole prefabs, and recording
//! hierarchies live in `fruits_asset_loading`:
//!
//! ```ignore
//! use fruits_engine::*;
//!
//! let serializers = res.get::<SerializersResource>().unwrap();
//! let mut on_err = |err| println!("{err}");
//! let mut ctx = serializers.to_ctx(&mut on_err);
//!
//! let components = serialize_components(source, ctx.as_mut(), ent.as_ref());
//! deserialize_prefab_components(&components, target, ctx, ent.as_mut());
//! ```
//!
//! # How to maintain
//!
//! #### Data model
//!
//! A [`Prefab`] holds [`PrefabEntities`] — an `FfiIndexMap` from a prefab-local entity id
//! (`u64`) to an `FfiVec` of [`PrefabComponent`], each pairing a `component_id` string with
//! the component's [`SerializedValue`] payload — and [`PrefabDependencies`], per-asset-type
//! maps from asset key to the loaded `AssetHandle` (textures, meshes, materials, audio clips,
//! fonts, and other prefabs). All of these are `#[repr(C)]`. The ids are local to the prefab:
//! instantiation creates one real entity per id and resolves entity references inside
//! component data through those ids. [`PrefabComponent`] derives [`Serializable`], and
//! `SerializedValue` is serializable as-is, so the component payloads pass through prefab
//! (de)serialization verbatim.
//!
//! #### The component id is the type name
//!
//! [`serialize_components`] walks the entity's components with `get_all_components`, stores
//! each one's type-info name (its `std::any::type_name`) as the `component_id`, serializes it
//! with [`SerializerCtx::serialize_any`], and sorts the result by `component_id` so the output
//! is stable. [`deserialize_component`] goes the other way: if the entity already has a
//! component with that id, it deserializes the payload into it in place at the given path;
//! otherwise [`SerializerCtx::deserialize_default_any`] looks the serializer up by the id,
//! creates the component from the serializer's default, fills it from the payload, and it is
//! attached with `add_component_any`. It returns `false` if no serializer matches or the
//! component cannot be added; [`deserialize_prefab_components`] logs such components and
//! moves on.
//!
//! #### Where the rest of the pipeline lives
//!
//! This crate only owns the format and the per-component conversion. Reading prefab files,
//! loading their dependencies, instantiating them into the world, and recording entity
//! hierarchies into prefabs live in `fruits_asset_loading`, and the
//! `AssetStorageResource<Prefab>` is inserted there by `add_asset_module_to`.
//! [`serialize_prefab_single_entity`] and [`deserialize_prefab_components`] are marked `todo`:
//! the former always stores the entity under id `0` and leaves the dependencies empty.

use fruits_asset_storage::AssetHandle;
use fruits_audio::AudioClip;
use fruits_ecs::*;
use fruits_ffi::{FfiIndexMap, FfiString};
use fruits_ui::Font;
use fruits_render_core::{StandardMaterial, StandardMesh, StandardTexture};
use fruits_serialization::*;

#[repr(C)]
#[derive(Default, Debug, Clone)]
pub struct PrefabEntityComponents(pub FfiIndexMap<FfiString, SerializedValue>);

impl<S: Copy + SerializerCtxState<SerializedValue>> Serializable<S> for PrefabEntityComponents {
    fn serialize(&self, ctx: SerializerCtx<S>, path: &str) -> SerializedValue {
        self.0.serialize(ctx, path)
    }

    fn deserialize(&mut self, ctx: SerializerCtx<S>, path: &str, serialized: &SerializedValue) {
        self.0.deserialize(ctx, path, serialized)
    }
}

impl From<FfiIndexMap<FfiString, SerializedValue>> for PrefabEntityComponents {
    fn from(value: FfiIndexMap<FfiString, SerializedValue>) -> Self {
        Self(value)
    }
}

#[repr(C)]
#[derive(Default, Debug, Clone)]
pub struct PrefabEntities(pub FfiIndexMap<u64, PrefabEntityComponents>);

impl<S: Copy + SerializerCtxState<PrefabEntityComponents>> Serializable<S> for PrefabEntities {
    fn serialize(&self, ctx: SerializerCtx<S>, path: &str) -> SerializedValue {
        self.0.serialize(ctx, path)
    }

    fn deserialize(&mut self, ctx: SerializerCtx<S>, path: &str, serialized: &SerializedValue) {
        self.0.deserialize(ctx, path, serialized)
    }
}

impl From<FfiIndexMap<u64, PrefabEntityComponents>> for PrefabEntities {
    fn from(value: FfiIndexMap<u64, PrefabEntityComponents>) -> Self {
        Self(value)
    }
}

#[repr(C)]
#[derive(Default, Debug, Clone)]
pub struct PrefabDependencies {
    pub textures: FfiIndexMap<FfiString, AssetHandle<StandardTexture>>,
    pub meshes: FfiIndexMap<FfiString, AssetHandle<StandardMesh>>,
    pub materials: FfiIndexMap<FfiString, AssetHandle<StandardMaterial>>,
    pub audio_clips: FfiIndexMap<FfiString, AssetHandle<AudioClip>>,
    pub fonts: FfiIndexMap<FfiString, AssetHandle<Font>>,
    pub prefabs: FfiIndexMap<FfiString, AssetHandle<Prefab>>,
}

#[repr(C)]
#[derive(Debug, Clone)]
pub struct Prefab {
    pub entities: PrefabEntities,
    pub dependencies: PrefabDependencies,
}

impl Prefab {
    pub fn empty() -> Self {
        Self {
            entities: Default::default(),
            dependencies: Default::default(),
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
        entities: PrefabEntities([
            (0, PrefabEntityComponents(serialize_components(entity, serializer_ctx, entities)))
        ].into_iter().collect()),
        // todo
        dependencies: PrefabDependencies::default(),
    }
}

// todo
pub fn deserialize_prefab_components(
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

    entities.get_all_components(entity, |component| {
        components.push((
            component.type_info().short().name().into(),
            serializer_ctx.serialize_any(component, ""),
        ));
    });

    // todo: implement sorting directly on FfiIndexMap
    components.sort_by(|l, r| l.0.cmp(&r.0));
    components.into_iter().collect()
}

pub fn deserialize_component(
    mut entities: EntitiesHolderMut,
    entity: EntityId,
    id: &str,
    mut serializer_ctx: SerializerCtx<TransSerializerCtxState>,
    path: &str,
    serialized: &SerializedValue,
) -> bool {
    let mut component = None;

    entities.get_all_components_mut(entity, |entity_component| {
        if component.is_some() || entity_component.type_info().short().name() != id {
            return;
        }

        component = Some(entity_component);
    });

    if let Some(c) = component {
        serializer_ctx.deserialize_any(c, path, serialized);
        return true;
    }

    let Some(component) = serializer_ctx.deserialize_default_any(id, path, serialized) else {
        return false;
    };

    entities.add_component_any(entity, component).is_ok()
}
