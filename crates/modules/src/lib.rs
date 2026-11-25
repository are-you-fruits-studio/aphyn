//! # aphyn_modules
//!
//! Bundles the engine's built-in subsystems into a single registration step, so an
//! app starts from a world that already has collision, physics, transforms, UI, rendering, audio,
//! animation, asset loading, serialization, and reflection wired together.
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
//! #### Reflecting the common types
//!
//! The default modules also insert a read-only [`ReflectionRegistryResource`] built by
//! [`register_common_reflectors`] from the same types. Use it to resolve fields once and keep them:
//!
//! ```ignore
//! use aphyn::*;
//!
//! fn position_field(data: WorldDataMut) -> Option<FfiArc<ReflectField>> {
//!     let res = data.resources();
//!     let registry = res.get::<ReflectionRegistryResource>()?;
//!     registry.get_of::<LocalTransform>()?.as_struct()?.field("position").cloned()
//! }
//! ```
//!
//! # How to maintain
//!
//! #### Default module set and ordering
//!
//! [`add_defult_modules_to`] is the assembly point. It registers the collision, physics
//! (whose step runs alone in the `FixedUpdate` schedule, so it needs no ordering here),
//! transform, UI, render (`aphyn_render_features`), audio, animation, and asset
//! subsystems through their own `add_*_module_to`
//! helpers, inserts a [`SerializersResource`] filled by [`register_common_transserializers`],
//! then constrains the [`Update`](aphyn_ecs::Schedule::Update) schedule so the
//! [`SYSTEM_GROUP_COLLISION`](aphyn_collision::SYSTEM_GROUP_COLLISION) group runs before
//! [`SYSTEM_GROUP_TRANSFORM`](aphyn_transform::SYSTEM_GROUP_TRANSFORM), which runs before
//! [`SYSTEM_GROUP_RENDER`](aphyn_render_features::SYSTEM_GROUP_RENDER): colliders settle, transforms
//! propagate, the frame renders. Physics interpolation
//! ([`SYSTEM_GROUP_PHYSICS_INTERPOLATION`](aphyn_physics::SYSTEM_GROUP_PHYSICS_INTERPOLATION)) runs before
//! the transform group and its restore
//! ([`SYSTEM_GROUP_PHYSICS_RESTORE`](aphyn_physics::SYSTEM_GROUP_PHYSICS_RESTORE)) after it, so the
//! shown body pose propagates to children. Animation
//! ([`SYSTEM_GROUP_ANIMATION`](aphyn_animation::SYSTEM_GROUP_ANIMATION)) also runs before the transform
//! group, so animated transforms propagate in the same frame. In `Start` it orders `SYSTEM_GROUP_RENDER` (which inserts the built-in
//! shaders and material templates) before `SYSTEM_GROUP_ASSETS`, so asset files can reference them. The name carries an upstream misspelling (`defult`);
//! callers must match it exactly.
//!
//! #### Shared serializer registry
//!
//! [`SerializersResource`] (defined in `aphyn_serialization_ecs`) is a newtype over a
//! [`TransSerializerRegistry`] that derefs to it. It is filled here, with the world, because
//! this crate is the one that depends on every subsystem whose types it registers.
//! [`register_common_transserializers`] registers a [`StandardSerializer`] for exactly the
//! types the engine needs: every engine component (transform, render, UI, audio, animation, collision, physics,
//! [`NameComponent`]), the texture/mesh/shader/audio/animation-clip asset metadata types, the `Material` and
//! `MaterialTemplate` assets the asset loaders read (with `GpuShaderDesc` and its parts), and, recursively, the type of every field they contain, generic instantiations
//! included (`Vec2<FfiOption<UiVal>>`, `[[f32; 3]; 3]` behind `Mat3<f32>`, `f64` behind
//! `Quat<f32>`, ...). The list is maintained by hand: adding a component or a field type means
//! adding it here, otherwise (de)serialization reports `MissingSerializer`. The `AssetHandle<T>`,
//! `EntityId` and direct asset save/load serializers come from `aphyn_asset_loading`'s
//! `register_asset_loading_serializers`, called first; they read their per-operation data from the context's `ContextData`.
//!
//! Since `aphyn_serialization` sits below the subsystem crates, most of these types derive
//! `Serializable` in their own crate; the `aphyn_ffi`/`aphyn_math` and primitive types get
//! their impls in `aphyn_serialization`.
//!
//! #### Shared reflection registry
//!
//! [`register_common_reflectors`] mirrors [`register_common_transserializers`]: every type listed
//! there is registered in a `ReflectionRegistryBuilder`, which [`add_defult_modules_to`] builds into
//! the [`ReflectionRegistryResource`] (defined in `aphyn_reflection_ecs`). Each type implements
//! `Reflectable` in its own crate, mostly by derive; `Material` hides its GPU state.
//! `MaterialData` is a map of `MaterialDataEntry`s whose `value` accessor bumps the entry's change
//! snapshot when borrowed mutably; `CollisionLayer`, `LineBoundType`, `MeshCollider`, `EntityId` and
//! `AssetHandle<T>` are reflected as values with default and clone factories (their invariants
//! live in their own serializers). Generic instantiations are registered with
//! `register_with_factories`, since their generic impls can't name `Default`/`Clone` of `T`. The
//! registry is immutable, so a type added later needs a rebuilt registry
//! (`aphyn_reflection_ecs::extend_reflection_registry`). Unit tests check that every field and
//! element type referenced by a registered reflector is registered too, that every value type can
//! be cloned, that every component has a default factory, and that cloning and variant
//! construction work on a few engine types.
//!
//! #### FPS counter
//!
//! `fps_counter` keeps an `FpsResource` holding a sliding window of up to 100 recent
//! frame durations. Its system records the time between successive runs, pushes each
//! duration into the window (dropping the oldest past 100), and once per second
//! averages the window to derive frame time and FPS, which it prints. The resource's
//! value accessors are currently commented out (`todo`), so the measured numbers are
//! only printed, not exposed to other systems.

use aphyn_asset_storage::AssetHandle;
use aphyn_animation::{
    AnimatedLeaf, AnimationClip, AnimationClipAssetMetadata, AnimationComponent, AnimationKeyMetadata, AnimationTrackMetadata,
    AnimationWrapMode, KeyInterpolation,
};
use aphyn_audio::{AudioClip, AudioClipAssetMetadata, AudioSource};
use aphyn_collision::{
    ColliderComponent, ColliderShape, CollisionAabb, CollisionBox, CollisionLayer, CollisionLine, CollisionShape, CollisionSphere,
    LineBoundType, MeshCollider,
};
use aphyn_ecs::{EntityId, Schedule, WorldBehaviorBuilderMut, WorldDataMut};
use aphyn_ffi::{FfiIndexMap, FfiOption, FfiSmallString, FfiString, FfiVec};
use aphyn_math::{Mat, Mat3, Quat, Vec2, Vec3, Vec4};
use aphyn_physics::{
    PhysicsMaterial, PhysicsMaterialComponent, RigidbodyComponent, RigidbodyForces, RigidbodyInterpolationComponent,
    RigidbodyInterpolationState, RigidbodyType,
};
use aphyn_reflection::*;
use aphyn_reflection_ecs::ReflectionRegistryResource;
use aphyn_render::{
    Camera, CameraProjection, ComputeShaderAssetMetadata, CoordinateSpaceType, Material, MaterialData, MaterialDataEntry,
    MaterialDataValue, MaterialInput, MaterialPassTarget, MaterialTemplate, MaterialTemplatePass, MaterialTextureBinding,
    MaterialTextureFallback, MaterialValueBinding, RenderSpace, Shader, ShaderAssetMetadata, StandardMesh,
    StandardMeshAssetMetadata, StandardTexture, StandardTextureAssetMetadata, StandardTextureFormat, StandardVertex,
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
    LocalTransform, NameComponent, ParentComponent, RectChildAlignComponent, StaticComponent, UiDirection, UiSpacing, UiUnit, UiVal,
};
use aphyn_ui::{
    ChildrenRectMaskComponent, Font, HorizontalAlign, ImageComponent, ImageFillSettings, TextComponent, UiLineComponent, VerticalAlign,
};

pub mod fps_counter;

// todo: bundle registration params
pub fn add_defult_modules_to(mut data: WorldDataMut, mut behavior: WorldBehaviorBuilderMut, render: &mut RenderFeatureRegistry) {
    aphyn_collision::add_collision_module_to(data.as_mut(), behavior.as_mut(), render);
    aphyn_physics::add_physics_module_to(data.as_mut(), behavior.as_mut(), render);
    aphyn_transform::add_transform_module_to(data.as_mut(), behavior.as_mut());
    aphyn_ui::add_ui_module_to(data.as_mut(), behavior.as_mut(), render);
    aphyn_render_features::add_render_features_module_to(data.as_mut(), behavior.as_mut(), render);
    aphyn_audio::add_audio_module_to(data.as_mut(), behavior.as_mut(), render);
    aphyn_animation::add_animation_module_to(data.as_mut(), behavior.as_mut(), render);
    aphyn_asset_loading::add_asset_module_to(data.as_mut(), behavior.as_mut(), render);

    data.resources_mut().insert({
        let mut serializers = SerializersResource::default();
        register_common_transserializers(&mut *serializers);
        serializers
    });

    data.resources_mut().insert({
        let mut reflectors = ReflectionRegistryBuilder::new();
        register_common_reflectors(&mut reflectors);
        ReflectionRegistryResource(reflectors.build())
    });

    behavior
        .get_mut(Schedule::Update)
        .order_group(aphyn_collision::SYSTEM_GROUP_COLLISION)
        .before_group(aphyn_transform::SYSTEM_GROUP_TRANSFORM)
        .before_group(aphyn_render_features::SYSTEM_GROUP_RENDER);
    behavior
        .get_mut(Schedule::Update)
        .order_group(aphyn_physics::SYSTEM_GROUP_PHYSICS_INTERPOLATION)
        .before_group(aphyn_transform::SYSTEM_GROUP_TRANSFORM);
    behavior
        .get_mut(Schedule::Update)
        .order_group(aphyn_transform::SYSTEM_GROUP_TRANSFORM)
        .before_group(aphyn_physics::SYSTEM_GROUP_PHYSICS_RESTORE);

    behavior
        .get_mut(Schedule::Update)
        .order_group(aphyn_animation::SYSTEM_GROUP_ANIMATION)
        .before_group(aphyn_transform::SYSTEM_GROUP_TRANSFORM);

    // asset files may reference the built-in shaders and material templates
    behavior
        .get_mut(Schedule::Start)
        .order_group(aphyn_render_features::SYSTEM_GROUP_RENDER)
        .before_group(aphyn_asset_loading::SYSTEM_GROUP_ASSETS);
}

pub fn register_common_transserializers(serializer: &mut TransSerializerRegistry) {
    // todo: automate it better (make components expose an api to register all their dependent types that implement Serializable).
    aphyn_asset_loading::register_asset_loading_serializers(serializer);

    // components
    register_transserializer::<NameComponent>(serializer);
    register_transserializer::<GlobalTransform>(serializer);
    register_transserializer::<LocalTransform>(serializer);
    register_transserializer::<ParentComponent>(serializer);
    register_transserializer::<ChildComponent>(serializer);
    register_transserializer::<StaticComponent>(serializer);
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
    register_transserializer::<UiLineComponent>(serializer);
    register_transserializer::<AudioSource>(serializer);
    register_transserializer::<AnimationComponent>(serializer);
    register_transserializer::<ColliderComponent>(serializer);
    register_transserializer::<RigidbodyComponent>(serializer);
    register_transserializer::<PhysicsMaterialComponent>(serializer);
    register_transserializer::<RigidbodyInterpolationComponent>(serializer);

    // asset metadata
    register_transserializer::<StandardMeshAssetMetadata>(serializer);
    register_transserializer::<StandardTextureAssetMetadata>(serializer);
    register_transserializer::<ShaderAssetMetadata>(serializer);
    register_transserializer::<ComputeShaderAssetMetadata>(serializer);
    register_transserializer::<AudioClipAssetMetadata>(serializer);
    register_transserializer::<AnimationClipAssetMetadata>(serializer);
    register_transserializer::<AnimationTrackMetadata>(serializer);
    register_transserializer::<FfiVec<AnimationTrackMetadata>>(serializer);
    register_transserializer::<AnimationKeyMetadata>(serializer);
    register_transserializer::<FfiVec<AnimationKeyMetadata>>(serializer);
    register_transserializer::<AnimatedLeaf>(serializer);
    register_transserializer::<KeyInterpolation>(serializer);
    register_transserializer::<AnimationWrapMode>(serializer);
    register_transserializer::<SerializedValue>(serializer);
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
    register_transserializer::<FfiVec<Vec2<f32>>>(serializer);
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
    register_transserializer::<RigidbodyInterpolationState>(serializer);
    register_transserializer::<PhysicsMaterial>(serializer);

    register_transserializer::<FfiVec<EntityId>>(serializer);
}

pub fn register_common_reflectors(reflectors: &mut ReflectionRegistryBuilder) {
    // components
    reflectors
        .register::<NameComponent>()
        .register::<GlobalTransform>()
        .register::<LocalTransform>()
        .register::<ParentComponent>()
        .register::<ChildComponent>()
        .register::<StaticComponent>()
        .register::<GlobalRectComponent>()
        .register::<LocalRectComponent>()
        .register::<GlobalDisableableComponent>()
        .register::<LocalDisableableComponent>()
        .register::<RectChildAlignComponent>()
        .register::<BatchedMeshComponent>()
        .register::<MeshComponent>()
        .register::<MaterialComponent>()
        .register::<StandardLightComponent>()
        .register::<CameraComponent>()
        .register::<TextComponent>()
        .register::<ImageComponent>()
        .register::<ChildrenRectMaskComponent>()
        .register::<UiLineComponent>()
        .register::<AudioSource>()
        .register::<AnimationComponent>()
        .register::<AnimationWrapMode>()
        .register::<ColliderComponent>()
        .register::<RigidbodyComponent>()
        .register::<PhysicsMaterialComponent>()
        .register::<RigidbodyInterpolationComponent>();

    // asset metadata
    reflectors
        .register::<StandardMeshAssetMetadata>()
        .register::<StandardTextureAssetMetadata>()
        .register::<ShaderAssetMetadata>()
        .register::<ComputeShaderAssetMetadata>()
        .register::<AudioClipAssetMetadata>()
        .register::<Material>()
        .register::<MaterialTemplate>();

    // field types
    reflectors
        .register::<bool>()
        .register::<u8>()
        .register::<u16>()
        .register::<u64>()
        .register::<usize>()
        .register_with_factories::<FfiOption<usize>>()
        .register::<f32>()
        .register::<f64>()
        .register::<FfiString>()
        .register::<FfiSmallString>()
        .register_with_factories::<FfiOption<f32>>()
        .register_with_factories::<FfiVec<u16>>()
        .register_with_factories::<[f32; 2]>()
        .register_with_factories::<[f32; 3]>()
        .register_with_factories::<[f32; 4]>()
        .register_with_factories::<[[f32; 3]; 3]>()
        .register_with_factories::<Vec2<f32>>()
        .register_with_factories::<FfiVec<Vec2<f32>>>()
        .register_with_factories::<Vec3<f32>>()
        .register_with_factories::<Vec4<f32>>()
        .register_with_factories::<[Vec3<f32>; 3]>()
        .register_with_factories::<Mat3<f32>>()
        .register_with_factories::<Quat<f32>>()
        .register::<UiVal>()
        .register::<UiUnit>()
        .register::<UiSpacing>()
        .register::<UiDirection>()
        .register_with_factories::<Vec2<UiVal>>()
        .register_with_factories::<FfiOption<UiVal>>()
        .register_with_factories::<Vec2<FfiOption<UiVal>>>()
        .register::<MaterialData>()
        .register::<MaterialDataEntry>()
        .register::<MaterialDataValue>()
        .register::<StandardVertex>()
        .register_with_factories::<FfiVec<StandardVertex>>()
        .register::<Camera>()
        .register::<CameraProjection>()
        .register::<RenderSpace>()
        .register::<MaterialInput>()
        .register_with_factories::<FfiIndexMap<FfiString, MaterialInput>>()
        .register::<MaterialTextureFallback>()
        .register::<MaterialPassTarget>()
        .register::<MaterialTemplatePass>()
        .register_with_factories::<FfiVec<MaterialTemplatePass>>()
        .register::<MaterialValueBinding>()
        .register_with_factories::<FfiVec<MaterialValueBinding>>()
        .register::<MaterialTextureBinding>()
        .register_with_factories::<FfiVec<MaterialTextureBinding>>()
        .register::<GpuShaderDesc>()
        .register::<GpuPrimitiveTopology>()
        .register::<GpuFrontFace>()
        .register::<GpuCullMode>()
        .register::<GpuBlendState>()
        .register::<GpuBlendComponent>()
        .register::<GpuBlendFactor>()
        .register::<GpuBlendOperation>()
        .register::<GpuColorWrites>()
        .register::<GpuCompareFunction>()
        .register::<CoordinateSpaceType>()
        .register::<StandardTextureFormat>()
        .register::<HorizontalAlign>()
        .register::<VerticalAlign>()
        .register::<ImageFillSettings>()
        .register::<CollisionShape>()
        .register::<CollisionLine>()
        .register::<CollisionAabb>()
        .register::<CollisionBox>()
        .register::<CollisionSphere>()
        .register::<LineBoundType>()
        .register::<ColliderShape>()
        .register::<CollisionLayer>()
        .register::<MeshCollider>()
        .register::<RigidbodyType>()
        .register::<RigidbodyForces>()
        .register::<RigidbodyInterpolationState>()
        .register::<PhysicsMaterial>()
        .register::<EntityId>()
        .register_with_factories::<FfiVec<EntityId>>()
        .register::<u32>()
        .register_with_factories::<FfiVec<u32>>()
        .register::<AssetHandle<StandardMesh>>()
        .register::<AssetHandle<StandardTexture>>()
        .register::<AssetHandle<Shader>>()
        .register::<AssetHandle<Material>>()
        .register::<AssetHandle<MaterialTemplate>>()
        .register::<AssetHandle<Font>>()
        .register::<AssetHandle<AudioClip>>()
        .register::<AssetHandle<AnimationClip>>();
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

#[cfg(test)]
mod tests {
    use std::any::type_name;

    use aphyn_ffi::{FfiAny, FfiAnyRef};
    use aphyn_reflection::*;

    use super::*;

    fn common_registry() -> ReflectionRegistry {
        let mut reflectors = ReflectionRegistryBuilder::new();
        register_common_reflectors(&mut reflectors);
        reflectors.build()
    }

    #[test]
    fn common_reflectors_reference_only_registered_types() {
        assert_eq!(common_registry().missing_dst_types(), Vec::<&str>::new());
    }

    #[test]
    fn value_reflectors_have_clone_factories() {
        let registry = common_registry();
        let missing = registry
            .iter()
            .filter(|r| r.as_value().is_some() && !r.has_clone())
            .map(|r| r.type_name())
            .collect::<Vec<_>>();

        assert_eq!(missing, Vec::<&str>::new());
    }

    #[test]
    fn components_have_default_factories() {
        let registry = common_registry();
        let components = [
            type_name::<NameComponent>(),
            type_name::<GlobalTransform>(),
            type_name::<LocalTransform>(),
            type_name::<ParentComponent>(),
            type_name::<ChildComponent>(),
            type_name::<StaticComponent>(),
            type_name::<GlobalRectComponent>(),
            type_name::<LocalRectComponent>(),
            type_name::<GlobalDisableableComponent>(),
            type_name::<LocalDisableableComponent>(),
            type_name::<RectChildAlignComponent>(),
            type_name::<BatchedMeshComponent>(),
            type_name::<MeshComponent>(),
            type_name::<MaterialComponent>(),
            type_name::<StandardLightComponent>(),
            type_name::<CameraComponent>(),
            type_name::<TextComponent>(),
            type_name::<ImageComponent>(),
            type_name::<ChildrenRectMaskComponent>(),
            type_name::<UiLineComponent>(),
            type_name::<AudioSource>(),
            type_name::<AnimationComponent>(),
            type_name::<ColliderComponent>(),
            type_name::<RigidbodyComponent>(),
            type_name::<PhysicsMaterialComponent>(),
            type_name::<RigidbodyInterpolationComponent>(),
        ];
        let missing = components
            .into_iter()
            .filter(|name| !registry.get(name).is_some_and(|r| r.has_default()))
            .collect::<Vec<_>>();

        assert_eq!(missing, Vec::<&str>::new());
    }

    #[test]
    fn reflect_clone_round_trips() {
        let registry = common_registry();

        let mut transform = LocalTransform::IDENTITY;
        transform.position = Vec3::new(1.0, 2.0, 3.0);
        let cloned = reflect_clone(&registry, FfiAnyRef::new(&transform)).unwrap();
        assert_eq!(cloned.downcast_ref::<LocalTransform>(), Some(&transform));

        let collider = ColliderComponent::new(CollisionShape::Point(Vec3::new(4.0, 5.0, 6.0)));
        let cloned = reflect_clone(&registry, FfiAnyRef::new(&collider)).unwrap();
        let ColliderShape::Primitive(CollisionShape::Point(point)) = cloned.downcast_ref::<ColliderComponent>().unwrap().shape else {
            panic!("the variant was not cloned");
        };
        assert_eq!(point, Vec3::new(4.0, 5.0, 6.0));

        // Material has no clone factory: default + fill
        let mut data = MaterialData::new();
        data.insert("roughness", MaterialDataValue::F32(0.25));
        let material = Material::new(AssetHandle::EMPTY, data);
        let cloned: FfiAny = reflect_clone(&registry, FfiAnyRef::new(&material)).unwrap();
        let cloned = cloned.downcast_ref::<Material>().unwrap();
        assert_eq!(cloned.data.get_f32("roughness"), Some(&0.25));
    }

    #[test]
    fn enum_variants_construct_from_defaults() {
        let registry = common_registry();
        let shape = registry.get_of::<CollisionShape>().unwrap().as_enum().unwrap();

        let sphere = reflect_construct_variant(&registry, shape.variant("Sphere").unwrap()).unwrap();
        assert!(matches!(sphere.downcast_ref::<CollisionShape>(), Some(CollisionShape::Sphere(_))));
    }

    #[test]
    fn animation_clip_metadata_serializes() {
        let mut registry = TransSerializerRegistry::new();
        register_common_transserializers(&mut registry);

        let key = AnimationKeyMetadata {
            time: 1.0,
            value: SerializedValue::Primitive(SerializedPrimitive::Float(2.0)),
            ..Default::default()
        };
        let track = AnimationTrackMetadata {
            component: "c".into(),
            field: "/x".into(),
            leaf: AnimatedLeaf::QuatEuler(2),
            value_type: "f32".into(),
            keys: vec![key].into(),
            ..Default::default()
        };
        let meta = AnimationClipAssetMetadata {
            tracks: vec![track].into(),
        };

        let mut errors = Vec::new();
        let serialized = registry.serialize(&meta, "", &mut |err: SerializationError| errors.push(err.to_string()));

        assert_eq!(errors, Vec::<String>::new());
        assert_eq!(serialized.to_json()["tracks"][0]["keys"][0]["time"], 1.0);

        let mut restored = AnimationClipAssetMetadata::default();
        registry.deserialize(&mut restored, "", &serialized, &mut |err: SerializationError| {
            errors.push(err.to_string())
        });

        assert_eq!(errors, Vec::<String>::new());
        assert_eq!(restored.tracks.len(), 1);
        assert_eq!(restored.tracks[0].keys[0].time, 1.0);
        assert_eq!(restored.tracks[0].leaf, AnimatedLeaf::QuatEuler(2));
    }
}
