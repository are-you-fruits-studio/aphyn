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
//! let mut ctx = app.init_ctx_mut();
//! add_default_modules_to(ctx.feature_ctx());
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
//! let mut ctx = app.init_ctx_mut();
//! add_default_modules_to(ctx.feature_ctx());
//! fps_counter::add_module_to(ctx.data.as_mut(), ctx.behavior.as_mut(), ctx.render);
//! app.run();
//! ```
//!
//! #### Making the common types serializable
//!
//! The default modules fill the init context's `TransSerializerRegistryBuilder` with the engine's
//! components and the types they contain (see below); the app builds it into the world's
//! read-only `SerializersResource` when it starts running. To serialize your own type through it,
//! register it and every field type not registered yet during init with [`register_transserializer`];
//! [`register_self_and_related_common_transserializers`] also registers the type's vector,
//! matrix, array, `Vec`/`FfiVec`, and `Option`/`FfiOption` wrappers:
//!
//! ```ignore
//! use aphyn::*;
//!
//! #[derive(Component, Serializable, Default)]
//! struct Health(f32);
//!
//! fn setup(ctx: AppInitCtx) {
//!     register_self_and_related_common_transserializers::<Health>(ctx.types, "my_game::Health", ctx.serializers);
//! }
//! ```
//!
//! #### Reflecting the common types
//!
//! The default modules also fill the init context's `ReflectionRegistryBuilder` through
//! [`register_common_reflectors`] with the same types; the app builds it into the world's read-only
//! `ReflectionRegistryResource`. Use it to resolve fields once and keep them:
//!
//! ```ignore
//! use aphyn::*;
//!
//! fn position_field(data: WorldDataMut) -> Option<FfiArc<ReflectField>> {
//!     let res = data.resources();
//!     let registry = res.get::<ReflectionRegistryResource>()?;
//!     registry.get_of::<LocalTransform>(data.types())?.as_struct()?.field("position").cloned()
//! }
//! ```
//!
//! # How to maintain
//!
//! #### Default module set and ordering
//!
//! [`add_default_modules_to`] is the assembly point. It registers the collision, physics
//! (whose step runs alone in the `FixedUpdate` schedule, so it needs no ordering here),
//! transform, UI, render (`aphyn_render_features`), audio, animation, and asset
//! subsystems through their own `add_*_module_to`
//! helpers, fills the init context's serializer builder through [`register_common_transserializers`]
//! and its reflection builder through [`register_common_reflectors`],
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
//! `SerializersResource` (defined in `aphyn_serialization_ecs`) is a newtype over the
//! [`TransSerializerRegistry`] that the app builds from the init context's
//! [`TransSerializerRegistryBuilder`] when it starts running. The builder is filled here because
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
//! Every registration names its type; the name is what asset files store. Engine types are
//! namespaced as `aphyn::<TypeName>` (`aphyn::LocalTransform`, `aphyn::FfiString`), primitives and
//! std types stay bare (`f32`, `bool`, `String`), and generic instantiations spell their arguments the
//! same way (`aphyn::Vec2<aphyn::FfiOption<aphyn::UiVal>>`, `[[f32; 3]; 3]`, `Vec<aphyn::EntityId>`);
//! `Mat3<f32>` is named `aphyn::Mat<3, f32>`, as the generic helpers spell it.
//! [`register_related_common_transserializers`] composes those names from the inner type's name.
//! Games should use their own prefix. Renaming a type breaks the asset files that use it.
//!
//! Since `aphyn_serialization` sits below the subsystem crates, most of these types derive
//! `Serializable` in their own crate; the `aphyn_ffi`/`aphyn_math` and primitive types get
//! their impls in `aphyn_serialization`.
//!
//! #### Shared reflection registry
//!
//! [`register_common_reflectors`] mirrors [`register_common_transserializers`]: every type listed
//! there is registered in the init context's `ReflectionRegistryBuilder`, which the app builds into
//! the `ReflectionRegistryResource` (defined in `aphyn_reflection_ecs`). Each type implements
//! `Reflectable` in its own crate, mostly by derive; `Material` hides its GPU state.
//! `MaterialData` is a map of `MaterialDataEntry`s whose `value` accessor bumps the entry's change
//! snapshot when borrowed mutably; `CollisionLayer`, `LineBoundType`, `MeshCollider`, `EntityId` and
//! `AssetHandle<T>` are reflected as values with default and clone factories (their invariants
//! live in their own serializers). Generic instantiations are registered with
//! `register_with_factories`, since their generic impls can't name `Default`/`Clone` of `T`. The
//! registry is immutable once built, so every type must be registered during app init. Unit tests
//! check that every field and element type referenced by a registered reflector is registered too, that every value type can
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
use aphyn_app::FeatureInitCtx;
use aphyn_ecs::{EntityId, Schedule, TypesRegistryCache};
use aphyn_ffi::{FfiIndexMap, FfiOption, FfiSmallString, FfiString, FfiVec};
use aphyn_math::{Mat, Mat3, Quat, Vec2, Vec3, Vec4};
use aphyn_physics::{
    PhysicsMaterial, PhysicsMaterialComponent, RigidbodyComponent, RigidbodyForces, RigidbodyInterpolationComponent,
    RigidbodyInterpolationState, RigidbodyType,
};
use aphyn_reflection::*;
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
    BatchedMeshComponent, CameraComponent, MaterialComponent, MeshComponent, StandardLightComponent,
};
use aphyn_serialization::*;
use aphyn_transform::{
    ChildComponent, GlobalDisableableComponent, GlobalRectComponent, GlobalTransform, LocalDisableableComponent, LocalRectComponent,
    LocalTransform, NameComponent, ParentComponent, RectChildAlignComponent, StaticComponent, UiDirection, UiSpacing, UiUnit, UiVal,
};
use aphyn_ui::{
    ChildrenRectMaskComponent, Font, HorizontalAlign, ImageComponent, ImageFillSettings, TextComponent, UiLineComponent, VerticalAlign,
};

pub mod fps_counter;

pub fn add_default_modules_to(ctx: FeatureInitCtx) {
    let FeatureInitCtx {
        mut data,
        types,
        mut behavior,
        render,
        serializers,
        reflection,
    } = ctx;

    aphyn_collision::add_collision_module_to(data.as_mut(), behavior.as_mut(), render);
    aphyn_physics::add_physics_module_to(data.as_mut(), behavior.as_mut(), render);
    aphyn_transform::add_transform_module_to(data.as_mut(), behavior.as_mut());
    aphyn_ui::add_ui_module_to(data.as_mut(), behavior.as_mut(), render);
    aphyn_render_features::add_render_features_module_to(data.as_mut(), behavior.as_mut(), render);
    aphyn_audio::add_audio_module_to(data.as_mut(), behavior.as_mut(), render);
    aphyn_animation::add_animation_module_to(data.as_mut(), behavior.as_mut(), render);
    aphyn_asset_loading::add_asset_module_to(data.as_mut(), behavior.as_mut(), render);

    register_common_transserializers(types, serializers);
    register_common_reflectors(types, reflection);

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

pub fn register_common_transserializers(types: &TypesRegistryCache, serializer: &mut TransSerializerRegistryBuilder) {
    // todo: automate it better (make components expose an api to register all their dependent types that implement Serializable).
    aphyn_asset_loading::register_asset_loading_serializers(types, serializer);

    // components
    register_transserializer::<NameComponent>(types, "aphyn::NameComponent", serializer);
    register_transserializer::<GlobalTransform>(types, "aphyn::GlobalTransform", serializer);
    register_transserializer::<LocalTransform>(types, "aphyn::LocalTransform", serializer);
    register_transserializer::<ParentComponent>(types, "aphyn::ParentComponent", serializer);
    register_transserializer::<ChildComponent>(types, "aphyn::ChildComponent", serializer);
    register_transserializer::<StaticComponent>(types, "aphyn::StaticComponent", serializer);
    register_transserializer::<GlobalRectComponent>(types, "aphyn::GlobalRectComponent", serializer);
    register_transserializer::<LocalRectComponent>(types, "aphyn::LocalRectComponent", serializer);
    register_transserializer::<GlobalDisableableComponent>(types, "aphyn::GlobalDisableableComponent", serializer);
    register_transserializer::<LocalDisableableComponent>(types, "aphyn::LocalDisableableComponent", serializer);
    register_transserializer::<RectChildAlignComponent>(types, "aphyn::RectChildAlignComponent", serializer);
    register_transserializer::<BatchedMeshComponent>(types, "aphyn::BatchedMeshComponent", serializer);
    register_transserializer::<MeshComponent>(types, "aphyn::MeshComponent", serializer);
    register_transserializer::<MaterialComponent>(types, "aphyn::MaterialComponent", serializer);
    register_transserializer::<StandardLightComponent>(types, "aphyn::StandardLightComponent", serializer);
    register_transserializer::<CameraComponent>(types, "aphyn::CameraComponent", serializer);
    register_transserializer::<TextComponent>(types, "aphyn::TextComponent", serializer);
    register_transserializer::<ImageComponent>(types, "aphyn::ImageComponent", serializer);
    register_transserializer::<ChildrenRectMaskComponent>(types, "aphyn::ChildrenRectMaskComponent", serializer);
    register_transserializer::<UiLineComponent>(types, "aphyn::UiLineComponent", serializer);
    register_transserializer::<AudioSource>(types, "aphyn::AudioSource", serializer);
    register_transserializer::<AnimationComponent>(types, "aphyn::AnimationComponent", serializer);
    register_transserializer::<ColliderComponent>(types, "aphyn::ColliderComponent", serializer);
    register_transserializer::<RigidbodyComponent>(types, "aphyn::RigidbodyComponent", serializer);
    register_transserializer::<PhysicsMaterialComponent>(types, "aphyn::PhysicsMaterialComponent", serializer);
    register_transserializer::<RigidbodyInterpolationComponent>(types, "aphyn::RigidbodyInterpolationComponent", serializer);

    // asset metadata
    register_transserializer::<StandardMeshAssetMetadata>(types, "aphyn::StandardMeshAssetMetadata", serializer);
    register_transserializer::<StandardTextureAssetMetadata>(types, "aphyn::StandardTextureAssetMetadata", serializer);
    register_transserializer::<ShaderAssetMetadata>(types, "aphyn::ShaderAssetMetadata", serializer);
    register_transserializer::<ComputeShaderAssetMetadata>(types, "aphyn::ComputeShaderAssetMetadata", serializer);
    register_transserializer::<AudioClipAssetMetadata>(types, "aphyn::AudioClipAssetMetadata", serializer);
    register_transserializer::<AnimationClipAssetMetadata>(types, "aphyn::AnimationClipAssetMetadata", serializer);
    register_transserializer::<AnimationTrackMetadata>(types, "aphyn::AnimationTrackMetadata", serializer);
    register_transserializer::<FfiVec<AnimationTrackMetadata>>(types, "aphyn::FfiVec<aphyn::AnimationTrackMetadata>", serializer);
    register_transserializer::<AnimationKeyMetadata>(types, "aphyn::AnimationKeyMetadata", serializer);
    register_transserializer::<FfiVec<AnimationKeyMetadata>>(types, "aphyn::FfiVec<aphyn::AnimationKeyMetadata>", serializer);
    register_transserializer::<AnimatedLeaf>(types, "aphyn::AnimatedLeaf", serializer);
    register_transserializer::<KeyInterpolation>(types, "aphyn::KeyInterpolation", serializer);
    register_transserializer::<AnimationWrapMode>(types, "aphyn::AnimationWrapMode", serializer);
    register_transserializer::<SerializedValue>(types, "aphyn::SerializedValue", serializer);
    register_transserializer::<Material>(types, "aphyn::Material", serializer);
    register_transserializer::<MaterialTemplate>(types, "aphyn::MaterialTemplate", serializer);

    // field types
    register_transserializer::<bool>(types, "bool", serializer);
    register_transserializer::<u8>(types, "u8", serializer);
    register_transserializer::<u16>(types, "u16", serializer);
    register_transserializer::<u64>(types, "u64", serializer);
    register_transserializer::<usize>(types, "usize", serializer);
    register_transserializer::<FfiOption<usize>>(types, "aphyn::FfiOption<usize>", serializer);
    register_transserializer::<f32>(types, "f32", serializer);
    register_transserializer::<f64>(types, "f64", serializer);
    register_transserializer::<FfiString>(types, "aphyn::FfiString", serializer);
    register_transserializer::<FfiSmallString>(types, "aphyn::FfiSmallString", serializer);
    register_transserializer::<FfiOption<f32>>(types, "aphyn::FfiOption<f32>", serializer);
    register_transserializer::<FfiVec<u16>>(types, "aphyn::FfiVec<u16>", serializer);
    register_transserializer::<[f32; 2]>(types, "[f32; 2]", serializer);
    register_transserializer::<[f32; 3]>(types, "[f32; 3]", serializer);
    register_transserializer::<[f32; 4]>(types, "[f32; 4]", serializer);
    register_transserializer::<[[f32; 3]; 3]>(types, "[[f32; 3]; 3]", serializer);
    register_transserializer::<Vec2<f32>>(types, "aphyn::Vec2<f32>", serializer);
    register_transserializer::<FfiVec<Vec2<f32>>>(types, "aphyn::FfiVec<aphyn::Vec2<f32>>", serializer);
    register_transserializer::<Vec3<f32>>(types, "aphyn::Vec3<f32>", serializer);
    register_transserializer::<Vec4<f32>>(types, "aphyn::Vec4<f32>", serializer);
    register_transserializer::<[Vec3<f32>; 3]>(types, "[aphyn::Vec3<f32>; 3]", serializer);
    register_transserializer::<Mat3<f32>>(types, "aphyn::Mat<3, f32>", serializer);
    register_transserializer::<Quat<f32>>(types, "aphyn::Quat<f32>", serializer);
    register_transserializer::<UiVal>(types, "aphyn::UiVal", serializer);
    register_transserializer::<UiUnit>(types, "aphyn::UiUnit", serializer);
    register_transserializer::<UiSpacing>(types, "aphyn::UiSpacing", serializer);
    register_transserializer::<UiDirection>(types, "aphyn::UiDirection", serializer);
    register_transserializer::<Vec2<UiVal>>(types, "aphyn::Vec2<aphyn::UiVal>", serializer);
    register_transserializer::<FfiOption<UiVal>>(types, "aphyn::FfiOption<aphyn::UiVal>", serializer);
    register_transserializer::<Vec2<FfiOption<UiVal>>>(types, "aphyn::Vec2<aphyn::FfiOption<aphyn::UiVal>>", serializer);
    register_transserializer::<MaterialData>(types, "aphyn::MaterialData", serializer);
    register_transserializer::<MaterialDataEntry>(types, "aphyn::MaterialDataEntry", serializer);
    register_transserializer::<MaterialDataValue>(types, "aphyn::MaterialDataValue", serializer);
    register_transserializer::<StandardVertex>(types, "aphyn::StandardVertex", serializer);
    register_transserializer::<FfiVec<StandardVertex>>(types, "aphyn::FfiVec<aphyn::StandardVertex>", serializer);
    register_transserializer::<Camera>(types, "aphyn::Camera", serializer);
    register_transserializer::<CameraProjection>(types, "aphyn::CameraProjection", serializer);
    register_transserializer::<RenderSpace>(types, "aphyn::RenderSpace", serializer);
    register_transserializer::<MaterialInput>(types, "aphyn::MaterialInput", serializer);
    register_transserializer::<FfiIndexMap<FfiString, MaterialInput>>(types, "aphyn::FfiIndexMap<aphyn::FfiString, aphyn::MaterialInput>", serializer);
    register_transserializer::<MaterialTextureFallback>(types, "aphyn::MaterialTextureFallback", serializer);
    register_transserializer::<MaterialPassTarget>(types, "aphyn::MaterialPassTarget", serializer);
    register_transserializer::<MaterialTemplatePass>(types, "aphyn::MaterialTemplatePass", serializer);
    register_transserializer::<FfiVec<MaterialTemplatePass>>(types, "aphyn::FfiVec<aphyn::MaterialTemplatePass>", serializer);
    register_transserializer::<MaterialValueBinding>(types, "aphyn::MaterialValueBinding", serializer);
    register_transserializer::<FfiVec<MaterialValueBinding>>(types, "aphyn::FfiVec<aphyn::MaterialValueBinding>", serializer);
    register_transserializer::<MaterialTextureBinding>(types, "aphyn::MaterialTextureBinding", serializer);
    register_transserializer::<FfiVec<MaterialTextureBinding>>(types, "aphyn::FfiVec<aphyn::MaterialTextureBinding>", serializer);
    register_transserializer::<GpuShaderDesc>(types, "aphyn::GpuShaderDesc", serializer);
    register_transserializer::<GpuPrimitiveTopology>(types, "aphyn::GpuPrimitiveTopology", serializer);
    register_transserializer::<GpuFrontFace>(types, "aphyn::GpuFrontFace", serializer);
    register_transserializer::<GpuCullMode>(types, "aphyn::GpuCullMode", serializer);
    register_transserializer::<GpuBlendState>(types, "aphyn::GpuBlendState", serializer);
    register_transserializer::<GpuBlendComponent>(types, "aphyn::GpuBlendComponent", serializer);
    register_transserializer::<GpuBlendFactor>(types, "aphyn::GpuBlendFactor", serializer);
    register_transserializer::<GpuBlendOperation>(types, "aphyn::GpuBlendOperation", serializer);
    register_transserializer::<GpuColorWrites>(types, "aphyn::GpuColorWrites", serializer);
    register_transserializer::<GpuCompareFunction>(types, "aphyn::GpuCompareFunction", serializer);
    register_transserializer::<CoordinateSpaceType>(types, "aphyn::CoordinateSpaceType", serializer);
    register_transserializer::<StandardTextureFormat>(types, "aphyn::StandardTextureFormat", serializer);
    register_transserializer::<HorizontalAlign>(types, "aphyn::HorizontalAlign", serializer);
    register_transserializer::<VerticalAlign>(types, "aphyn::VerticalAlign", serializer);
    register_transserializer::<ImageFillSettings>(types, "aphyn::ImageFillSettings", serializer);
    register_transserializer::<CollisionShape>(types, "aphyn::CollisionShape", serializer);
    register_transserializer::<CollisionLine>(types, "aphyn::CollisionLine", serializer);
    register_transserializer::<CollisionAabb>(types, "aphyn::CollisionAabb", serializer);
    register_transserializer::<CollisionBox>(types, "aphyn::CollisionBox", serializer);
    register_transserializer::<CollisionSphere>(types, "aphyn::CollisionSphere", serializer);
    register_transserializer::<LineBoundType>(types, "aphyn::LineBoundType", serializer);
    register_transserializer::<ColliderShape>(types, "aphyn::ColliderShape", serializer);
    register_transserializer::<CollisionLayer>(types, "aphyn::CollisionLayer", serializer);
    register_transserializer::<MeshCollider>(types, "aphyn::MeshCollider", serializer);
    register_transserializer::<RigidbodyType>(types, "aphyn::RigidbodyType", serializer);
    register_transserializer::<RigidbodyForces>(types, "aphyn::RigidbodyForces", serializer);
    register_transserializer::<RigidbodyInterpolationState>(types, "aphyn::RigidbodyInterpolationState", serializer);
    register_transserializer::<PhysicsMaterial>(types, "aphyn::PhysicsMaterial", serializer);

    register_transserializer::<FfiVec<EntityId>>(types, "aphyn::FfiVec<aphyn::EntityId>", serializer);
}

pub fn register_common_reflectors(types: &TypesRegistryCache, reflectors: &mut ReflectionRegistryBuilder) {
    // components
    reflectors
        .register::<NameComponent>(types)
        .register::<GlobalTransform>(types)
        .register::<LocalTransform>(types)
        .register::<ParentComponent>(types)
        .register::<ChildComponent>(types)
        .register::<StaticComponent>(types)
        .register::<GlobalRectComponent>(types)
        .register::<LocalRectComponent>(types)
        .register::<GlobalDisableableComponent>(types)
        .register::<LocalDisableableComponent>(types)
        .register::<RectChildAlignComponent>(types)
        .register::<BatchedMeshComponent>(types)
        .register::<MeshComponent>(types)
        .register::<MaterialComponent>(types)
        .register::<StandardLightComponent>(types)
        .register::<CameraComponent>(types)
        .register::<TextComponent>(types)
        .register::<ImageComponent>(types)
        .register::<ChildrenRectMaskComponent>(types)
        .register::<UiLineComponent>(types)
        .register::<AudioSource>(types)
        .register::<AnimationComponent>(types)
        .register::<AnimationWrapMode>(types)
        .register::<ColliderComponent>(types)
        .register::<RigidbodyComponent>(types)
        .register::<PhysicsMaterialComponent>(types)
        .register::<RigidbodyInterpolationComponent>(types);

    // asset metadata
    reflectors
        .register::<StandardMeshAssetMetadata>(types)
        .register::<StandardTextureAssetMetadata>(types)
        .register::<ShaderAssetMetadata>(types)
        .register::<ComputeShaderAssetMetadata>(types)
        .register::<AudioClipAssetMetadata>(types)
        .register::<Material>(types)
        .register::<MaterialTemplate>(types);

    // field types
    reflectors
        .register::<bool>(types)
        .register::<u8>(types)
        .register::<u16>(types)
        .register::<u64>(types)
        .register::<usize>(types)
        .register_with_factories::<FfiOption<usize>>(types)
        .register::<f32>(types)
        .register::<f64>(types)
        .register::<FfiString>(types)
        .register::<FfiSmallString>(types)
        .register_with_factories::<FfiOption<f32>>(types)
        .register_with_factories::<FfiVec<u16>>(types)
        .register_with_factories::<[f32; 2]>(types)
        .register_with_factories::<[f32; 3]>(types)
        .register_with_factories::<[f32; 4]>(types)
        .register_with_factories::<[[f32; 3]; 3]>(types)
        .register_with_factories::<Vec2<f32>>(types)
        .register_with_factories::<FfiVec<Vec2<f32>>>(types)
        .register_with_factories::<Vec3<f32>>(types)
        .register_with_factories::<Vec4<f32>>(types)
        .register_with_factories::<[Vec3<f32>; 3]>(types)
        .register_with_factories::<Mat3<f32>>(types)
        .register_with_factories::<Quat<f32>>(types)
        .register::<UiVal>(types)
        .register::<UiUnit>(types)
        .register::<UiSpacing>(types)
        .register::<UiDirection>(types)
        .register_with_factories::<Vec2<UiVal>>(types)
        .register_with_factories::<FfiOption<UiVal>>(types)
        .register_with_factories::<Vec2<FfiOption<UiVal>>>(types)
        .register::<MaterialData>(types)
        .register::<MaterialDataEntry>(types)
        .register::<MaterialDataValue>(types)
        .register::<StandardVertex>(types)
        .register_with_factories::<FfiVec<StandardVertex>>(types)
        .register::<Camera>(types)
        .register::<CameraProjection>(types)
        .register::<RenderSpace>(types)
        .register::<MaterialInput>(types)
        .register_with_factories::<FfiIndexMap<FfiString, MaterialInput>>(types)
        .register::<MaterialTextureFallback>(types)
        .register::<MaterialPassTarget>(types)
        .register::<MaterialTemplatePass>(types)
        .register_with_factories::<FfiVec<MaterialTemplatePass>>(types)
        .register::<MaterialValueBinding>(types)
        .register_with_factories::<FfiVec<MaterialValueBinding>>(types)
        .register::<MaterialTextureBinding>(types)
        .register_with_factories::<FfiVec<MaterialTextureBinding>>(types)
        .register::<GpuShaderDesc>(types)
        .register::<GpuPrimitiveTopology>(types)
        .register::<GpuFrontFace>(types)
        .register::<GpuCullMode>(types)
        .register::<GpuBlendState>(types)
        .register::<GpuBlendComponent>(types)
        .register::<GpuBlendFactor>(types)
        .register::<GpuBlendOperation>(types)
        .register::<GpuColorWrites>(types)
        .register::<GpuCompareFunction>(types)
        .register::<CoordinateSpaceType>(types)
        .register::<StandardTextureFormat>(types)
        .register::<HorizontalAlign>(types)
        .register::<VerticalAlign>(types)
        .register::<ImageFillSettings>(types)
        .register::<CollisionShape>(types)
        .register::<CollisionLine>(types)
        .register::<CollisionAabb>(types)
        .register::<CollisionBox>(types)
        .register::<CollisionSphere>(types)
        .register::<LineBoundType>(types)
        .register::<ColliderShape>(types)
        .register::<CollisionLayer>(types)
        .register::<MeshCollider>(types)
        .register::<RigidbodyType>(types)
        .register::<RigidbodyForces>(types)
        .register::<RigidbodyInterpolationState>(types)
        .register::<PhysicsMaterial>(types)
        .register::<EntityId>(types)
        .register_with_factories::<FfiVec<EntityId>>(types)
        .register::<u32>(types)
        .register_with_factories::<FfiVec<u32>>(types)
        .register::<AssetHandle<StandardMesh>>(types)
        .register::<AssetHandle<StandardTexture>>(types)
        .register::<AssetHandle<Shader>>(types)
        .register::<AssetHandle<Material>>(types)
        .register::<AssetHandle<MaterialTemplate>>(types)
        .register::<AssetHandle<Font>>(types)
        .register::<AssetHandle<AudioClip>>(types)
        .register::<AssetHandle<AnimationClip>>(types);
}

pub fn register_transserializer<T: 'static + Default + for<'a> Serializable<TransSerializerCtxState<'a>>>(
    types: &TypesRegistryCache,
    name: &str,
    serializer: &mut TransSerializerRegistryBuilder,
) {
    serializer.register(types, name, StandardSerializer::<T>::default());
}

pub fn register_self_and_related_common_transserializers<
    T: 'static + Default + for<'a> Serializable<TransSerializerCtxState<'a>>,
>(
    types: &TypesRegistryCache,
    name: &str,
    serializer: &mut TransSerializerRegistryBuilder,
) {
    serializer.register(types, name, StandardSerializer::<T>::default());
    register_related_common_transserializers::<T>(types, name, serializer)
}
pub fn register_related_common_transserializers<T: 'static + Default>(types: &TypesRegistryCache, name: &str, serializer: &mut TransSerializerRegistryBuilder) {
    serializer.register(types, &format!("aphyn::Vec2<{name}>"), StandardSerializer::<Vec2<T>>::default());
    serializer.register(types, &format!("aphyn::Vec3<{name}>"), StandardSerializer::<Vec3<T>>::default());
    serializer.register(types, &format!("aphyn::Vec4<{name}>"), StandardSerializer::<Vec4<T>>::default());
    // todo
    serializer.register(types, &format!("aphyn::Mat<0, {name}>"), StandardSerializer::<Mat<0, T>>::default());
    serializer.register(types, &format!("aphyn::Mat<1, {name}>"), StandardSerializer::<Mat<1, T>>::default());
    serializer.register(types, &format!("aphyn::Mat<2, {name}>"), StandardSerializer::<Mat<2, T>>::default());
    serializer.register(types, &format!("aphyn::Mat<3, {name}>"), StandardSerializer::<Mat<3, T>>::default());
    serializer.register(types, &format!("aphyn::Mat<4, {name}>"), StandardSerializer::<Mat<4, T>>::default());
    serializer.register(types, &format!("[{name}; 0]"), StandardSerializer::<[T; 0]>::default());
    serializer.register(types, &format!("[{name}; 1]"), StandardSerializer::<[T; 1]>::default());
    serializer.register(types, &format!("[{name}; 2]"), StandardSerializer::<[T; 2]>::default());
    serializer.register(types, &format!("[{name}; 3]"), StandardSerializer::<[T; 3]>::default());
    serializer.register(types, &format!("[{name}; 4]"), StandardSerializer::<[T; 4]>::default());
    serializer.register(types, &format!("[[{name}; 0]; 0]"), StandardSerializer::<[[T; 0]; 0]>::default());
    serializer.register(types, &format!("[[{name}; 0]; 1]"), StandardSerializer::<[[T; 0]; 1]>::default());
    serializer.register(types, &format!("[[{name}; 0]; 2]"), StandardSerializer::<[[T; 0]; 2]>::default());
    serializer.register(types, &format!("[[{name}; 0]; 3]"), StandardSerializer::<[[T; 0]; 3]>::default());
    serializer.register(types, &format!("[[{name}; 0]; 4]"), StandardSerializer::<[[T; 0]; 4]>::default());
    serializer.register(types, &format!("[[{name}; 1]; 0]"), StandardSerializer::<[[T; 1]; 0]>::default());
    serializer.register(types, &format!("[[{name}; 1]; 1]"), StandardSerializer::<[[T; 1]; 1]>::default());
    serializer.register(types, &format!("[[{name}; 1]; 2]"), StandardSerializer::<[[T; 1]; 2]>::default());
    serializer.register(types, &format!("[[{name}; 1]; 3]"), StandardSerializer::<[[T; 1]; 3]>::default());
    serializer.register(types, &format!("[[{name}; 1]; 4]"), StandardSerializer::<[[T; 1]; 4]>::default());
    serializer.register(types, &format!("[[{name}; 2]; 0]"), StandardSerializer::<[[T; 2]; 0]>::default());
    serializer.register(types, &format!("[[{name}; 2]; 1]"), StandardSerializer::<[[T; 2]; 1]>::default());
    serializer.register(types, &format!("[[{name}; 2]; 2]"), StandardSerializer::<[[T; 2]; 2]>::default());
    serializer.register(types, &format!("[[{name}; 2]; 3]"), StandardSerializer::<[[T; 2]; 3]>::default());
    serializer.register(types, &format!("[[{name}; 2]; 4]"), StandardSerializer::<[[T; 2]; 4]>::default());
    serializer.register(types, &format!("[[{name}; 3]; 0]"), StandardSerializer::<[[T; 3]; 0]>::default());
    serializer.register(types, &format!("[[{name}; 3]; 1]"), StandardSerializer::<[[T; 3]; 1]>::default());
    serializer.register(types, &format!("[[{name}; 3]; 2]"), StandardSerializer::<[[T; 3]; 2]>::default());
    serializer.register(types, &format!("[[{name}; 3]; 3]"), StandardSerializer::<[[T; 3]; 3]>::default());
    serializer.register(types, &format!("[[{name}; 3]; 4]"), StandardSerializer::<[[T; 3]; 4]>::default());
    serializer.register(types, &format!("[[{name}; 4]; 0]"), StandardSerializer::<[[T; 4]; 0]>::default());
    serializer.register(types, &format!("[[{name}; 4]; 1]"), StandardSerializer::<[[T; 4]; 1]>::default());
    serializer.register(types, &format!("[[{name}; 4]; 2]"), StandardSerializer::<[[T; 4]; 2]>::default());
    serializer.register(types, &format!("[[{name}; 4]; 3]"), StandardSerializer::<[[T; 4]; 3]>::default());
    serializer.register(types, &format!("[[{name}; 4]; 4]"), StandardSerializer::<[[T; 4]; 4]>::default());
    serializer.register(types, &format!("Vec<{name}>"), StandardSerializer::<Vec<T>>::default());
    serializer.register(types, &format!("aphyn::FfiVec<{name}>"), StandardSerializer::<FfiVec<T>>::default());
    serializer.register(types, &format!("Option<{name}>"), StandardSerializer::<Option<T>>::default());
    serializer.register(types, &format!("aphyn::FfiOption<{name}>"), StandardSerializer::<FfiOption<T>>::default());
}

#[cfg(test)]
mod tests {
    use std::any::type_name;

    use aphyn_any::*;
    use aphyn_reflection::*;

    use super::*;

    fn types() -> TypesRegistryCache {
        TypesRegistryCache::new(TypesRegistryAccessFfi::new())
    }

    fn common_registry_in(types: &TypesRegistryCache) -> ReflectionRegistry {
        let mut reflectors = ReflectionRegistryBuilder::new();
        register_common_reflectors(types, &mut reflectors);
        reflectors.build()
    }

    fn common_registry() -> ReflectionRegistry {
        common_registry_in(&types())
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
        let types = types();
        let registry = common_registry_in(&types);
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
            .filter(|name| {
                !types
                    .ffi()
                    .get_by_name(name)
                    .and_then(|info| registry.get(AnyTypeId::new(types.tag(), info.id)))
                    .is_some_and(|r| r.has_default())
            })
            .collect::<Vec<_>>();

        assert_eq!(missing, Vec::<&str>::new());
    }

    #[test]
    fn reflect_clone_round_trips() {
        let types = types();
        let registry = common_registry_in(&types);

        let mut transform = LocalTransform::IDENTITY;
        transform.position = Vec3::new(1.0, 2.0, 3.0);
        let cloned = reflect_clone(&registry, AnyRef::new(&transform, types.type_id_of())).unwrap();
        assert_eq!(cloned.downcast_ref(types.type_id_of::<LocalTransform>()), Some(&transform));

        let collider = ColliderComponent::new(CollisionShape::Point(Vec3::new(4.0, 5.0, 6.0)));
        let cloned = reflect_clone(&registry, AnyRef::new(&collider, types.type_id_of())).unwrap();
        let ColliderShape::Primitive(CollisionShape::Point(point)) = cloned.downcast_ref(types.type_id_of::<ColliderComponent>()).unwrap().shape else {
            panic!("the variant was not cloned");
        };
        assert_eq!(point, Vec3::new(4.0, 5.0, 6.0));

        // Material has no clone factory: default + fill
        let mut data = MaterialData::new();
        data.insert("roughness", MaterialDataValue::F32(0.25));
        let material = Material::new(AssetHandle::EMPTY, data);
        let cloned: AnyBox = reflect_clone(&registry, AnyRef::new(&material, types.type_id_of())).unwrap();
        let cloned = cloned.downcast_ref(types.type_id_of::<Material>()).unwrap();
        assert_eq!(cloned.data.get_f32("roughness"), Some(&0.25));
    }

    #[test]
    fn enum_variants_construct_from_defaults() {
        let types = types();
        let registry = common_registry_in(&types);
        let shape = registry.get_of::<CollisionShape>(&types).unwrap().as_enum().unwrap();

        let sphere = reflect_construct_variant(&registry, shape.variant("Sphere").unwrap()).unwrap();
        assert!(matches!(sphere.downcast_ref(types.type_id_of::<CollisionShape>()), Some(CollisionShape::Sphere(_))));
    }

    #[test]
    fn animation_clip_metadata_serializes() {
        let types = types();
        let mut builder = TransSerializerRegistryBuilder::new();
        register_common_transserializers(&types, &mut builder);
        let registry = builder.build();

        let key = AnimationKeyMetadata {
            time: 1.0,
            value: SerializedValue::Primitive(SerializedPrimitive::Float(2.0)),
            ..Default::default()
        };
        let track = AnimationTrackMetadata {
            component: "aphyn::LocalTransform".into(),
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
        let serialized = registry.serialize(&types, &meta, "", &mut |err: SerializationError| errors.push(err.to_string()));

        assert_eq!(errors, Vec::<String>::new());
        assert_eq!(serialized.to_json()["tracks"][0]["keys"][0]["time"], 1.0);

        let mut restored = AnimationClipAssetMetadata::default();
        registry.deserialize(&types, &mut restored, "", &serialized, &mut |err: SerializationError| {
            errors.push(err.to_string())
        });

        assert_eq!(errors, Vec::<String>::new());
        assert_eq!(restored.tracks.len(), 1);
        assert_eq!(restored.tracks[0].keys[0].time, 1.0);
        assert_eq!(restored.tracks[0].leaf, AnimatedLeaf::QuatEuler(2));
    }
}
