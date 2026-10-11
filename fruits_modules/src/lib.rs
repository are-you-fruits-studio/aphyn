//! # fruits_modules
//!
//! Bundles the engine's built-in subsystems into a single registration step, so an
//! app starts from a world that already has collision, transforms, UI, rendering, audio,
//! asset loading, and serialization wired together.
//!
//! # How to use
//!
//! #### Building a default world
//!
//! Register every default subsystem at once on a freshly created `App`
//! before populating the world. This is the call almost every app makes first:
//!
//! ```ignore
//! use fruits_engine::*;
//!
//! let mut app = App::new();
//! add_defult_modules_to(app.ecs_mut().as_mut());
//! // ... register your own systems and entities ...
//! app.run();
//! ```
//!
//! #### Printing the frame rate
//!
//! The FPS counter is opt-in — it is *not* part of the default modules. Add it to
//! print the averaged frame rate and frame time to stdout once per second:
//!
//! ```ignore
//! use fruits_engine::*;
//!
//! let mut app = App::new();
//! add_defult_modules_to(app.ecs_mut().as_mut());
//! fps_counter::add_module_to(app.ecs_mut().as_mut());
//! app.run();
//! ```
//!
//! #### Making the common types serializable
//!
//! The default modules fill the world's [`SerializersResource`] with the engine's common
//! types (see below). To serialize your own type through it, register it the same way;
//! [`register_self_and_related_common_transserializers`] also registers its vector, matrix,
//! quaternion, `Vec`/`FfiVec`, and `Option`/`FfiOption` wrappers:
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
//!     register_self_and_related_common_transserializers::<Health>(&mut **serializers);
//! }
//! ```
//!
//! # How to maintain
//!
//! #### Default module set and ordering
//!
//! [`add_defult_modules_to`] is the assembly point. It registers the collision,
//! transform, UI, render, audio, and asset subsystems through their own `add_*_module_to`
//! helpers, inserts a [`SerializersResource`] filled by [`register_common_transserializers`],
//! then constrains the [`Update`](fruits_ecs::Schedule::Update) schedule so the
//! [`SYSTEM_GROUP_COLLISION`](fruits_collision::SYSTEM_GROUP_COLLISION) group runs before
//! [`SYSTEM_GROUP_TRANSFORM`](fruits_transform::SYSTEM_GROUP_TRANSFORM), which runs before
//! [`SYSTEM_GROUP_RENDER`](fruits_render::SYSTEM_GROUP_RENDER): colliders settle, transforms
//! propagate, the frame renders. The name carries an upstream misspelling (`defult`);
//! callers must match it exactly.
//!
//! #### Shared serializer registry
//!
//! [`SerializersResource`] (defined in `fruits_serialization`) is a newtype over a `'static`
//! [`TransSerializerRegistry`] that derefs to it. It is filled here, with the world, because
//! this crate is the one that depends on every subsystem whose types it registers.
//! [`register_common_transserializers`] registers a [`StandardSerializer`] for the primitive
//! and string types, render-space and coordinate-space enums, the texture/mesh/material/audio
//! asset metadata types, [`DebugNameComponent`], and the transform hierarchy components, each
//! together with its related wrappers. For `AssetHandle<T>` it registers only the wrappers: the
//! handle itself is (de)serialized by the per-operation local serializers in
//! `fruits_asset_loading`, which layer over this registry. The related wrappers include
//! `Mat<0..=8, T>`, whose `Serializable` impl is still a `todo!()` in `fruits_serialization`.
//!
//! [`DebugNameComponent`] is a serializable component wrapping an `FfiSmallString` name.
//!
//! #### FPS counter
//!
//! `fps_counter` keeps an `FpsResource` holding a sliding window of up to 100 recent
//! frame durations. Its system records the time between successive runs, pushes each
//! duration into the window (dropping the oldest past 100), and once per second
//! averages the window to derive frame time and FPS, which it prints. The resource's
//! value accessors are currently commented out (`todo`), so the measured numbers are
//! only printed, not exposed to other systems.

use fruits_asset_storage::AssetHandle;
use fruits_audio::AudioClipAssetMetadata;
use fruits_ecs::{Component, Schedule, WorldBuilderMut};
use fruits_ffi::{FfiOption, FfiSmallString, FfiString, FfiVec};
use fruits_math::{Mat, Quat, Vec2, Vec3, Vec4};
use fruits_prefab::Prefab;
use fruits_ui::Font;
use fruits_render_core::{CoordinateSpaceType, RenderSpace, StandardMaterial, StandardMaterialAssetMetadata, StandardMesh, StandardMeshAssetMetadata, StandardTexture, StandardTextureAssetMetadata};
use fruits_serialization::*;
use fruits_transform::{ChildComponent, ParentComponent};

pub mod fps_counter;

pub fn add_defult_modules_to(mut world: WorldBuilderMut) {
    fruits_collision::add_collision_module_to(world.as_mut());
    fruits_transform::add_transform_module_to(world.as_mut());
    fruits_ui::add_ui_module_to(world.as_mut());
    fruits_render::add_render_module_to(world.as_mut());
    fruits_audio::add_audio_module_to(world.as_mut());
    fruits_asset_loading::add_asset_module_to(world.as_mut());

    world.data_mut().resources_mut().insert({
        let mut serializers = SerializersResource::default();
        register_common_transserializers(&mut *serializers);
        serializers
    });

    world
        .behavior_mut()
        .get_mut(Schedule::Update)
        .order_group(fruits_collision::SYSTEM_GROUP_COLLISION)
        .before_group(fruits_transform::SYSTEM_GROUP_TRANSFORM)
        .before_group(fruits_render::SYSTEM_GROUP_RENDER);
}

pub fn register_common_transserializers(serializer: &mut TransSerializerRegistry) {
    register_self_and_related_common_transserializers::<i8>(serializer);
    register_self_and_related_common_transserializers::<u8>(serializer);
    register_self_and_related_common_transserializers::<i16>(serializer);
    register_self_and_related_common_transserializers::<u16>(serializer);
    register_self_and_related_common_transserializers::<i32>(serializer);
    register_self_and_related_common_transserializers::<u32>(serializer);
    register_self_and_related_common_transserializers::<i64>(serializer);
    register_self_and_related_common_transserializers::<u64>(serializer);
    register_self_and_related_common_transserializers::<i128>(serializer);
    register_self_and_related_common_transserializers::<u128>(serializer);
    register_self_and_related_common_transserializers::<f32>(serializer);
    register_self_and_related_common_transserializers::<f64>(serializer);
    register_self_and_related_common_transserializers::<String>(serializer);
    register_self_and_related_common_transserializers::<FfiString>(serializer);
    register_self_and_related_common_transserializers::<char>(serializer);
    register_self_and_related_common_transserializers::<bool>(serializer);
    register_self_and_related_common_transserializers::<RenderSpace>(serializer);
    register_self_and_related_common_transserializers::<CoordinateSpaceType>(serializer);
    register_self_and_related_common_transserializers::<StandardMeshAssetMetadata>(serializer);
    register_self_and_related_common_transserializers::<AudioClipAssetMetadata>(serializer);
    register_self_and_related_common_transserializers::<StandardTextureAssetMetadata>(serializer);
    register_self_and_related_common_transserializers::<StandardMaterialAssetMetadata>(serializer);
    register_self_and_related_common_transserializers::<DebugNameComponent>(serializer);
    register_self_and_related_common_transserializers::<ParentComponent>(serializer);
    register_self_and_related_common_transserializers::<ChildComponent>(serializer);
    // todo?
    register_related_common_transserializers::<AssetHandle<StandardMaterial>>(serializer);
    register_related_common_transserializers::<AssetHandle<Font>>(serializer);
    register_related_common_transserializers::<AssetHandle<StandardTexture>>(serializer);
    register_related_common_transserializers::<AssetHandle<StandardMesh>>(serializer);
    register_related_common_transserializers::<AssetHandle<Prefab>>(serializer);
}

pub fn register_self_and_related_common_transserializers<T: 'static + Default + for<'a> Serializable<TransSerializerCtxState<'a>>>(serializer: &mut TransSerializerRegistry) {
    serializer.register(StandardSerializer::<T>::default());
    register_related_common_transserializers::<T>(serializer)
}
pub fn register_related_common_transserializers<T: 'static + Default>(serializer: &mut TransSerializerRegistry) {
    serializer.register(StandardSerializer::<Vec2<T>>::default());
    serializer.register(StandardSerializer::<Vec3<T>>::default());
    serializer.register(StandardSerializer::<Vec4<T>>::default());
    serializer.register(StandardSerializer::<Quat<T>>::default());
    serializer.register(StandardSerializer::<Mat<0, T>>::default());
    serializer.register(StandardSerializer::<Mat<1, T>>::default());
    serializer.register(StandardSerializer::<Mat<2, T>>::default());
    serializer.register(StandardSerializer::<Mat<3, T>>::default());
    serializer.register(StandardSerializer::<Mat<4, T>>::default());
    serializer.register(StandardSerializer::<Mat<5, T>>::default());
    serializer.register(StandardSerializer::<Mat<6, T>>::default());
    serializer.register(StandardSerializer::<Mat<7, T>>::default());
    serializer.register(StandardSerializer::<Mat<8, T>>::default());
    serializer.register(StandardSerializer::<Vec<T>>::default());
    serializer.register(StandardSerializer::<FfiVec<T>>::default());
    serializer.register(StandardSerializer::<Option<T>>::default());
    serializer.register(StandardSerializer::<FfiOption<T>>::default());
}

#[repr(transparent)]
#[derive(Component, Copy, Clone, Hash, PartialEq, Eq, PartialOrd, Ord, Default, Serializable)]
pub struct DebugNameComponent(pub FfiSmallString);