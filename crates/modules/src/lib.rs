//! # aphyn_modules
//!
//! Bundles the engine's built-in subsystems into a single registration step, so an
//! app starts from a world that already has collision, physics, transforms, UI, rendering, audio,
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
//! use aphyn::*;
//!
//! let mut app = App::new();
//! let ctx = app.init_ctx_mut();
//! add_defult_modules_to(ctx.data, ctx.behavior, ctx.render);
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
//! use aphyn::*;
//!
//! let mut app = App::new();
//! let AppInitCtx { mut data, mut behavior, render, .. } = app.init_ctx_mut();
//! add_defult_modules_to(data.as_mut(), behavior.as_mut(), render);
//! fps_counter::add_module_to(data.as_mut(), behavior.as_mut(), render);
//! app.run();
//! ```
//!
//! #### Making the common types serializable
//!
//! The default modules fill the world's [`SerializersResource`] with the engine's components
//! and the types they contain (see below). To serialize your own type through it, register it
//! and every field type not registered yet with [`register_transserializer`];
//! [`register_self_and_related_common_transserializers`] also registers the type's vector,
//! matrix, array, `Vec`/`FfiVec`, and `Option`/`FfiOption` wrappers:
//!
//! ```ignore
//! use aphyn::*;
//!
//! #[derive(Component, Serializable, Default)]
//! struct Health(f32);
//!
//! fn register_serializers(mut data: WorldDataMut) {
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
//! [`add_defult_modules_to`] is the assembly point. It registers the collision, physics
//! (whose step runs alone in the `FixedUpdate` schedule, so it needs no ordering here),
//! transform, UI, render (`aphyn_render_features`), audio, and asset
//! subsystems through their own `add_*_module_to`
//! helpers, inserts a [`SerializersResource`] filled by [`register_common_transserializers`],
//! then constrains the [`Update`](aphyn_ecs::Schedule::Update) schedule so the
//! [`SYSTEM_GROUP_COLLISION`](aphyn_collision::SYSTEM_GROUP_COLLISION) group runs before
//! [`SYSTEM_GROUP_TRANSFORM`](aphyn_transform::SYSTEM_GROUP_TRANSFORM), which runs before
//! [`SYSTEM_GROUP_RENDER`](aphyn_render_features::SYSTEM_GROUP_RENDER): colliders settle, transforms
//! propagate, the frame renders. In `Start` it orders `SYSTEM_GROUP_RENDER` (which inserts the built-in
//! shaders and material templates) before `SYSTEM_GROUP_ASSETS`, so asset files can reference them. The name carries an upstream misspelling (`defult`);
//! callers must match it exactly.
//!
//! #### Shared serializer registry
//!
//! [`SerializersResource`] (defined in `aphyn_serialization_ecs`) is a newtype over a `'static`
//! [`TransSerializerRegistry`] that derefs to it. It is filled here, with the world, because
//! this crate is the one that depends on every subsystem whose types it registers.
//! [`register_common_transserializers`] registers a [`StandardSerializer`] for exactly the
//! types the engine needs: every engine component (transform, render, UI, audio, collision, physics,
//! [`DebugNameComponent`]), the texture/mesh/shader/audio asset metadata types, the `Material` and
//! `MaterialTemplate` assets the asset loaders read (with `GpuShaderDesc` and its parts), and, recursively, the type of every field they contain, generic instantiations
//! included (`Vec2<FfiOption<UiVal>>`, `[[f32; 3]; 3]` behind `Mat3<f32>`, `f64` behind
//! `Quat<f32>`, ...). The list is maintained by hand: adding a component or a field type means
//! adding it here, otherwise (de)serialization reports `MissingSerializer`. `AssetHandle<T>` and
//! `EntityId` are deliberately absent: they are (de)serialized by the per-operation local
//! serializers in `aphyn_asset_loading`, which layer over this registry.
//!
//! Since `aphyn_serialization` sits below the subsystem crates, most of these types derive
//! `Serializable` in their own crate; the `aphyn_ffi`/`aphyn_math` and primitive types get
//! their impls in `aphyn_serialization`.
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

use aphyn_audio::{AudioClipAssetMetadata, AudioSource};
use aphyn_collision::{
    ColliderComponent, ColliderShape, CollisionAabb, CollisionBox, CollisionLayer, CollisionLine, CollisionShape, CollisionSphere,
    LineBoundType, MeshCollider,
};
use aphyn_ecs::{Component, Schedule, WorldBehaviorBuilderMut, WorldDataMut};
use aphyn_ffi::{FfiIndexMap, FfiOption, FfiSmallString, FfiString, FfiVec};
use aphyn_math::{Mat, Mat3, Quat, Vec2, Vec3, Vec4};
use aphyn_physics::{PhysicsMaterial, PhysicsMaterialComponent, RigidbodyComponent, RigidbodyForces, RigidbodyType};
use aphyn_render::{
    Camera, CameraProjection, CoordinateSpaceType, Material, MaterialData, MaterialDataEntry, MaterialDataValue, MaterialInput,
    MaterialPassTarget, MaterialTemplate, MaterialTemplatePass, MaterialTextureBinding, MaterialTextureFallback,
    MaterialValueBinding, RenderSpace, ShaderAssetMetadata, StandardMeshAssetMetadata, StandardTextureAssetMetadata,
    StandardTextureFormat, StandardVertex,
};
use aphyn_render_core::{
    GpuBlendComponent, GpuBlendFactor, GpuBlendOperation, GpuBlendState, GpuColorWrites, GpuCompareFunction, GpuCullMode,
    GpuFrontFace, GpuPrimitiveTopology, GpuShaderDesc,
};
use aphyn_render_features::{
    BatchedMeshComponent, CameraComponent, MaterialComponent, MeshComponent, RenderFeatureRegistry, StandardLightComponent,
};
use aphyn_serialization::*;
use aphyn_serialization_ecs::SerializersResource;
use aphyn_transform::{
    ChildComponent, GlobalDisableableComponent, GlobalRectComponent, GlobalTransform, LocalDisableableComponent, LocalRectComponent,
    LocalTransform, ParentComponent, RectChildAlignComponent, UiDirection, UiSpacing, UiUnit, UiVal,
};
use aphyn_ui::{ChildrenRectMaskComponent, HorizontalAlign, ImageComponent, ImageFillSettings, TextComponent, VerticalAlign};

pub mod fps_counter;

// todo: bundle registration params
pub fn add_defult_modules_to(mut data: WorldDataMut, mut behavior: WorldBehaviorBuilderMut, render: &mut RenderFeatureRegistry) {
    aphyn_collision::add_collision_module_to(data.as_mut(), behavior.as_mut(), render);
    aphyn_physics::add_physics_module_to(data.as_mut(), behavior.as_mut(), render);
    aphyn_transform::add_transform_module_to(data.as_mut(), behavior.as_mut());
    aphyn_ui::add_ui_module_to(data.as_mut(), behavior.as_mut(), render);
    aphyn_render_features::add_render_features_module_to(data.as_mut(), behavior.as_mut(), render);
    aphyn_audio::add_audio_module_to(data.as_mut(), behavior.as_mut(), render);
    aphyn_asset_loading::add_asset_module_to(data.as_mut(), behavior.as_mut(), render);

    data.resources_mut().insert({
        let mut serializers = SerializersResource::default();
        register_common_transserializers(&mut *serializers);
        serializers
    });

    behavior
        .get_mut(Schedule::Update)
        .order_group(aphyn_collision::SYSTEM_GROUP_COLLISION)
        .before_group(aphyn_transform::SYSTEM_GROUP_TRANSFORM)
        .before_group(aphyn_render_features::SYSTEM_GROUP_RENDER);

    // asset files may reference the built-in shaders and material templates
    behavior
        .get_mut(Schedule::Start)
        .order_group(aphyn_render_features::SYSTEM_GROUP_RENDER)
        .before_group(aphyn_asset_loading::SYSTEM_GROUP_ASSETS);
}

pub fn register_common_transserializers(serializer: &mut TransSerializerRegistry) {
    // todo: automate it better (make components expose an api to register all their dependent types that implement Serializable).
    // components
    register_transserializer::<DebugNameComponent>(serializer);
    register_transserializer::<GlobalTransform>(serializer);
    register_transserializer::<LocalTransform>(serializer);
    register_transserializer::<ParentComponent>(serializer);
    register_transserializer::<ChildComponent>(serializer);
    register_transserializer::<GlobalRectComponent>(serializer);
    register_transserializer::<LocalRectComponent>(serializer);
    register_transserializer::<GlobalDisableableComponent>(serializer);
    register_transserializer::<LocalDisableableComponent>(serializer);
    register_transserializer::<RectChildAlignComponent>(serializer);
    register_transserializer::<BatchedMeshComponent>(serializer);
    register_transserializer::<MeshComponent>(serializer);
    register_transserializer::<MaterialComponent>(serializer);
    register_transserializer::<StandardLightComponent>(serializer);
    register_transserializer::<CameraComponent>(serializer);
    register_transserializer::<TextComponent>(serializer);
    register_transserializer::<ImageComponent>(serializer);
    register_transserializer::<ChildrenRectMaskComponent>(serializer);
    register_transserializer::<AudioSource>(serializer);
    register_transserializer::<ColliderComponent>(serializer);
    register_transserializer::<RigidbodyComponent>(serializer);
    register_transserializer::<PhysicsMaterialComponent>(serializer);

    // asset metadata
    register_transserializer::<StandardMeshAssetMetadata>(serializer);
    register_transserializer::<StandardTextureAssetMetadata>(serializer);
    register_transserializer::<ShaderAssetMetadata>(serializer);
    register_transserializer::<AudioClipAssetMetadata>(serializer);
    register_transserializer::<Material>(serializer);
    register_transserializer::<MaterialTemplate>(serializer);

    // field types
    register_transserializer::<bool>(serializer);
    register_transserializer::<u8>(serializer);
    register_transserializer::<u16>(serializer);
    register_transserializer::<u64>(serializer);
    register_transserializer::<usize>(serializer);
    register_transserializer::<FfiOption<usize>>(serializer);
    register_transserializer::<f32>(serializer);
    register_transserializer::<f64>(serializer);
    register_transserializer::<FfiString>(serializer);
    register_transserializer::<FfiSmallString>(serializer);
    register_transserializer::<FfiOption<f32>>(serializer);
    register_transserializer::<FfiVec<u16>>(serializer);
    register_transserializer::<[f32; 2]>(serializer);
    register_transserializer::<[f32; 3]>(serializer);
    register_transserializer::<[f32; 4]>(serializer);
    register_transserializer::<[[f32; 3]; 3]>(serializer);
    register_transserializer::<Vec2<f32>>(serializer);
    register_transserializer::<Vec3<f32>>(serializer);
    register_transserializer::<Vec4<f32>>(serializer);
    register_transserializer::<[Vec3<f32>; 3]>(serializer);
    register_transserializer::<Mat3<f32>>(serializer);
    register_transserializer::<Quat<f32>>(serializer);
    register_transserializer::<UiVal>(serializer);
    register_transserializer::<UiUnit>(serializer);
    register_transserializer::<UiSpacing>(serializer);
    register_transserializer::<UiDirection>(serializer);
    register_transserializer::<Vec2<UiVal>>(serializer);
    register_transserializer::<FfiOption<UiVal>>(serializer);
    register_transserializer::<Vec2<FfiOption<UiVal>>>(serializer);
    register_transserializer::<MaterialData>(serializer);
    register_transserializer::<MaterialDataEntry>(serializer);
    register_transserializer::<MaterialDataValue>(serializer);
    register_transserializer::<StandardVertex>(serializer);
    register_transserializer::<FfiVec<StandardVertex>>(serializer);
    register_transserializer::<Camera>(serializer);
    register_transserializer::<CameraProjection>(serializer);
    register_transserializer::<RenderSpace>(serializer);
    register_transserializer::<MaterialInput>(serializer);
    register_transserializer::<FfiIndexMap<FfiString, MaterialInput>>(serializer);
    register_transserializer::<MaterialTextureFallback>(serializer);
    register_transserializer::<MaterialPassTarget>(serializer);
    register_transserializer::<MaterialTemplatePass>(serializer);
    register_transserializer::<FfiVec<MaterialTemplatePass>>(serializer);
    register_transserializer::<MaterialValueBinding>(serializer);
    register_transserializer::<FfiVec<MaterialValueBinding>>(serializer);
    register_transserializer::<MaterialTextureBinding>(serializer);
    register_transserializer::<FfiVec<MaterialTextureBinding>>(serializer);
    register_transserializer::<GpuShaderDesc>(serializer);
    register_transserializer::<GpuPrimitiveTopology>(serializer);
    register_transserializer::<GpuFrontFace>(serializer);
    register_transserializer::<GpuCullMode>(serializer);
    register_transserializer::<GpuBlendState>(serializer);
    register_transserializer::<GpuBlendComponent>(serializer);
    register_transserializer::<GpuBlendFactor>(serializer);
    register_transserializer::<GpuBlendOperation>(serializer);
    register_transserializer::<GpuColorWrites>(serializer);
    register_transserializer::<GpuCompareFunction>(serializer);
    register_transserializer::<CoordinateSpaceType>(serializer);
    register_transserializer::<StandardTextureFormat>(serializer);
    register_transserializer::<HorizontalAlign>(serializer);
    register_transserializer::<VerticalAlign>(serializer);
    register_transserializer::<ImageFillSettings>(serializer);
    register_transserializer::<CollisionShape>(serializer);
    register_transserializer::<CollisionLine>(serializer);
    register_transserializer::<CollisionAabb>(serializer);
    register_transserializer::<CollisionBox>(serializer);
    register_transserializer::<CollisionSphere>(serializer);
    register_transserializer::<LineBoundType>(serializer);
    register_transserializer::<ColliderShape>(serializer);
    register_transserializer::<CollisionLayer>(serializer);
    register_transserializer::<MeshCollider>(serializer);
    register_transserializer::<RigidbodyType>(serializer);
    register_transserializer::<RigidbodyForces>(serializer);
    register_transserializer::<PhysicsMaterial>(serializer);
}

pub fn register_transserializer<T: 'static + Default + for<'a> Serializable<TransSerializerCtxState<'a>>>(
    serializer: &mut TransSerializerRegistry,
) {
    serializer.register(StandardSerializer::<T>::default());
}

pub fn register_self_and_related_common_transserializers<
    T: 'static + Default + for<'a> Serializable<TransSerializerCtxState<'a>>,
>(
    serializer: &mut TransSerializerRegistry,
) {
    serializer.register(StandardSerializer::<T>::default());
    register_related_common_transserializers::<T>(serializer)
}
pub fn register_related_common_transserializers<T: 'static + Default>(serializer: &mut TransSerializerRegistry) {
    serializer.register(StandardSerializer::<Vec2<T>>::default());
    serializer.register(StandardSerializer::<Vec3<T>>::default());
    serializer.register(StandardSerializer::<Vec4<T>>::default());
    // todo
    serializer.register(StandardSerializer::<Mat<0, T>>::default());
    serializer.register(StandardSerializer::<Mat<1, T>>::default());
    serializer.register(StandardSerializer::<Mat<2, T>>::default());
    serializer.register(StandardSerializer::<Mat<3, T>>::default());
    serializer.register(StandardSerializer::<Mat<4, T>>::default());
    serializer.register(StandardSerializer::<[T; 0]>::default());
    serializer.register(StandardSerializer::<[T; 1]>::default());
    serializer.register(StandardSerializer::<[T; 2]>::default());
    serializer.register(StandardSerializer::<[T; 3]>::default());
    serializer.register(StandardSerializer::<[T; 4]>::default());
    serializer.register(StandardSerializer::<[[T; 0]; 0]>::default());
    serializer.register(StandardSerializer::<[[T; 0]; 1]>::default());
    serializer.register(StandardSerializer::<[[T; 0]; 2]>::default());
    serializer.register(StandardSerializer::<[[T; 0]; 3]>::default());
    serializer.register(StandardSerializer::<[[T; 0]; 4]>::default());
    serializer.register(StandardSerializer::<[[T; 1]; 0]>::default());
    serializer.register(StandardSerializer::<[[T; 1]; 1]>::default());
    serializer.register(StandardSerializer::<[[T; 1]; 2]>::default());
    serializer.register(StandardSerializer::<[[T; 1]; 3]>::default());
    serializer.register(StandardSerializer::<[[T; 1]; 4]>::default());
    serializer.register(StandardSerializer::<[[T; 2]; 0]>::default());
    serializer.register(StandardSerializer::<[[T; 2]; 1]>::default());
    serializer.register(StandardSerializer::<[[T; 2]; 2]>::default());
    serializer.register(StandardSerializer::<[[T; 2]; 3]>::default());
    serializer.register(StandardSerializer::<[[T; 2]; 4]>::default());
    serializer.register(StandardSerializer::<[[T; 3]; 0]>::default());
    serializer.register(StandardSerializer::<[[T; 3]; 1]>::default());
    serializer.register(StandardSerializer::<[[T; 3]; 2]>::default());
    serializer.register(StandardSerializer::<[[T; 3]; 3]>::default());
    serializer.register(StandardSerializer::<[[T; 3]; 4]>::default());
    serializer.register(StandardSerializer::<[[T; 4]; 0]>::default());
    serializer.register(StandardSerializer::<[[T; 4]; 1]>::default());
    serializer.register(StandardSerializer::<[[T; 4]; 2]>::default());
    serializer.register(StandardSerializer::<[[T; 4]; 3]>::default());
    serializer.register(StandardSerializer::<[[T; 4]; 4]>::default());
    serializer.register(StandardSerializer::<Vec<T>>::default());
    serializer.register(StandardSerializer::<FfiVec<T>>::default());
    serializer.register(StandardSerializer::<Option<T>>::default());
    serializer.register(StandardSerializer::<FfiOption<T>>::default());
}

#[repr(transparent)]
#[derive(Component, Copy, Clone, Hash, PartialEq, Eq, PartialOrd, Ord, Default, Serializable)]
pub struct DebugNameComponent(pub FfiSmallString);
