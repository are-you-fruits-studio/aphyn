//! # fruits_render
//!
//! Draws the contents of the world to the screen — lit and unlit meshes with PBR materials,
//! CPU-built batched geometry, and debug gizmo lines — followed by optional HDR
//! post-processing (exposure, bloom, ACES color grading), making up the engine's standard
//! rendering subsystem.
//!
//! # How to use
//!
//! Rendering is enabled by registering the module, which the engine's default-modules setup
//! (`fruits_modules::add_defult_modules_to`) already does. Once registered, an entity is drawn
//! by giving it the right components; the systems in this crate pick them up automatically each
//! frame. World-space content is projected through the single entity carrying a
//! [`CameraComponent`]; without one the world-space projection stays the identity. UI text and
//! images are drawn by `fruits_ui`, which produces [`BatchedMeshComponent`]s for this crate.
//!
//! #### Drawing a mesh
//!
//! Attach a [`StandardMeshComponent`] and a [`StandardMaterialComponent`] (plus a transform) to
//! an entity. Entities sharing the same mesh and material are drawn together in one instanced
//! draw:
//!
//! ```ignore
//! use fruits_engine::*;
//!
//! ec.add_component(entity, StandardMeshComponent { mesh: mesh.clone() }).ok().unwrap();
//! ec.add_component(entity, StandardMaterialComponent { material: material.clone() }).ok().unwrap();
//! ec.add_component(entity, GlobalTransform::default()).ok().unwrap();
//! ec.add_component(entity, LocalTransform::default()).ok().unwrap();
//! ```
//!
//! #### Placing the camera
//!
//! Give one entity a [`CameraComponent`] and a transform. The camera's transform is the eye
//! position and orientation; its `fov` is in radians:
//!
//! ```ignore
//! use fruits_engine::*;
//!
//! let camera = ec.create_entity();
//! ec.add_component(camera, GlobalTransform::default()).ok().unwrap();
//! ec.add_component(camera, LocalTransform {
//!     position: Vec3::new(0.0, 0.0, -5.0),
//!     ..Default::default()
//! }).ok().unwrap();
//! ec.add_component(camera, CameraComponent {
//!     near: 0.1,
//!     far: 1_000.0,
//!     fov: 90_f32.to_radians(),
//! }).ok().unwrap();
//! ```
//!
//! #### Lighting the scene
//!
//! Lit materials are shaded by every entity carrying a [`StandardLightComponent`] (up to
//! [`LIGHTS_COUNT_MAX`]). A point or spot light sits at its transform's position; spot and
//! directional lights point along their transform's local `-Y` axis:
//!
//! ```ignore
//! use fruits_engine::*;
//!
//! let sun = ec.create_entity();
//! ec.add_component(sun, GlobalTransform::default()).ok().unwrap();
//! ec.add_component(sun, LocalTransform {
//!     rotation: Quat::rotation_x(30.0_f64.to_radians()),
//!     ..Default::default()
//! }).ok().unwrap();
//! ec.add_component(sun, StandardLightComponent::Directional {
//!     color: Vec3::splat(3.0),
//! }).ok().unwrap();
//! ```
//!
//! #### Creating a material
//!
//! Create a material through the [`RenderApiResource`](fruits_render_core::RenderApiResource)
//! and store it in the `AssetStorageResource<StandardMaterial>`. `alpha_threshold` decides the
//! draw path: `Some(_)` is an opaque, alpha-tested surface, `None` a blended transparent one.
//! `space` selects the coordinate space the geometry is interpreted in (`World`, `Window` in
//! pixels, or `Clip`):
//!
//! ```ignore
//! use fruits_engine::*;
//!
//! let render_api = res.get::<RenderApiResource>().unwrap();
//! let material = render_api.create_material(Default::default(), StandardMaterialAssetMetadata {
//!     space: RenderSpace::World,
//!     color: Vec4::new(1.0, 0.5, 0.2, 1.0),
//!     is_lit: true,
//!     alpha_threshold: Some(0.5).into(),
//!     ..Default::default()
//! });
//!
//! let handle = res.get_mut::<AssetStorageResource<StandardMaterial>>().unwrap().insert(material);
//! ```
//!
//! #### Turning on post-processing
//!
//! Exposure, bloom, and color grading are off by default. Enable them by editing their
//! resources, from setup code or from a system at runtime:
//!
//! ```ignore
//! use fruits_engine::*;
//!
//! fn enable_post_processing(
//!     mut exposure: ResMut<ExposureResource>,
//!     mut bloom: ResMut<BloomResource>,
//!     mut color_grading: ResMut<ColorGradingResource>,
//! ) {
//!     exposure.is_enabled = true;
//!     exposure.exposure = 0.5; // in stops: the image is multiplied by 2^exposure
//!
//!     bloom.is_enabled = true;
//!     bloom.threshold = 1.0;
//!
//!     color_grading.ty = Some(ColorGradingType::Aces);
//! }
//! ```
//!
//! #### Drawing debug gizmo lines
//!
//! [`GizmosResource`] collects lines to draw for the current frame. Pick a space with
//! [`space`](GizmosResource::space) and push a [`GizmoLine`]; the lines are drawn and removed
//! each frame, so push them every frame they should appear:
//!
//! ```ignore
//! use fruits_engine::*;
//!
//! fn draw(mut gizmos: ResMut<GizmosResource>) {
//!     gizmos.space(RenderSpace::World).push(GizmoLine {
//!         start: Vec3::splat(0.0),
//!         end: Vec3::new(1.0, 0.0, 0.0),
//!         color: Vec4::new(1.0, 0.0, 0.0, 1.0),
//!     });
//! }
//! ```
//!
//! # How to maintain
//!
//! #### Registration and frame order
//!
//! [`add_render_module_to`] inserts the asset storages for materials, meshes, and textures and
//! the user-facing resources ([`GizmosResource`], [`ScreenSpaceResource`], [`BloomResource`],
//! [`ExposureResource`], [`ColorGradingResource`]), and registers its systems under the
//! [`SYSTEM_GROUP_RENDER`] group. [`Schedule::Start`] builds the
//! render targets, the post-processing resources, the gizmo resources, and finally
//! [`create_standard_render_resource`]. Each [`Schedule::Update`]
//! runs the inner [`SYSTEM_GROUP_RENDER_INTERNAL`] group and then
//! [`render_main_target_to_surface_system`]. Inside that group the explicit order is: recreate
//! any render target whose size no longer matches the surface (main, depth, transparent,
//! exposure, bloom, color grading), clear the main, depth, and transparent targets, update the
//! camera matrix, the lights buffer, and the global uniform, draw opaque geometry (instanced
//! then batched), draw transparent geometry (instanced then batched), composite the transparent
//! target, apply exposure, bloom, and color grading, and finally draw gizmos. Every pass records
//! its own command encoder and submits it immediately.
//!
//! #### Render targets and presentation
//!
//! Everything is drawn into [`MainRenderTargetResource`], an offscreen `Rgba16Float` HDR texture
//! the size of the surface, with a `Depth32Float` [`DepthTextureResource`].
//! [`render_main_target_to_surface_system`] acquires the surface texture, copies the main target
//! onto it with a fullscreen triangle (`shader_render_surface.wgsl`, nearest sampling, `REPLACE`
//! blend), and presents it; if the surface texture cannot be acquired the frame is skipped. The
//! `recreate_*` systems compare each target against the current surface size and only rebuild
//! when it changed, replacing the resource in place.
//!
//! #### Two geometry paths: instanced and batched
//!
//! Geometry reaches the GPU two ways. The **instanced** path ([`render_opaque_instanced`],
//! [`render_transparent_instanced`]) groups entities by their
//! ([`StandardMeshComponent`], [`StandardMaterialComponent`]) pair and issues indexed instanced
//! draws, writing the per-instance model matrices into a reused instance buffer in chunks of
//! [`INSTANCES_PER_DRAW_MAX`]. The **batched** path ([`render_opaque_batched`],
//! [`render_transparent_batched`]) is for [`BatchedMeshComponent`]: it groups entities by
//! material, transforms each indexed vertex on the CPU into world space (and converts vertex
//! colors from sRGB to linear), writes them into a shared non-indexed vertex buffer, and
//! flushes whenever the buffer fills ([`TRIANGLES_PER_BATCHED_DRAW_MAX`] triangles). Both paths
//! skip entities whose `GlobalDisableableComponent` is set, treat a missing transform as the
//! identity, and skip entities whose mesh or material handle does not resolve. The instanced and
//! batched functions are near-duplicates (see the `todo` notes). [`StandardRenderComponent`] is
//! defined but not read by any system.
//!
//! #### Opaque vs. transparent compositing
//!
//! A material's `alpha_threshold` splits the two: `Some` materials are drawn opaque into the
//! main target (depth-write on, `REPLACE` blend, alpha-tested with `discard` in the shader);
//! `None` materials are transparent. Transparent draws target an offscreen `Rgba16Float`
//! [`TransparentTargetTextureResource`] with additive blending and depth-test but no
//! depth-write, and [`render_transparent_final_system`] then composites that target over the
//! main target with a fullscreen triangle (`shader_transparent_final.wgsl`) using alpha
//! blending.
//!
//! #### Shaders, materials, and lights
//!
//! The standard shader is generated as WGSL source at pipeline-creation time by
//! [`shader_standard`], which concatenates code fragments and branches on
//! `is_lit`/`is_transparent`; [`StandardRenderResource`] holds the four resulting pipelines plus
//! the transparent-composite one. Bind group 0 is the global group (the
//! `StandardUniformGlobal` with the camera position and light count, plus the lights storage
//! buffer); bind group 1 is the material's. `get_render_data` picks the pipeline from
//! `is_lit` / `alpha_threshold` and rewrites the material uniform before every draw: color and
//! emission color are raised to the power 2.2 (sRGB to linear), and the world-to-clip matrix
//! comes from the material's `RenderSpace` — `World` uses the camera matrix, `Window` uses
//! [`create_window_to_clip_matrix`] (pixel coordinates with the near/far from
//! [`ScreenSpaceResource`]), and `Clip` uses the identity. The lit path samples the normal map
//! through a TBN matrix, scales metallic and roughness by their textures, starts from the
//! emission term, and adds a Cook-Torrance BRDF contribution per light; there is no ambient
//! term. The unlit path outputs the textured color and ignores emission.
//!
//! [`update_lights_buffer`] converts every [`StandardLightComponent`] with its transform (a
//! missing transform is the identity) into a `StandardGenericLight` via
//! [`light_from_components`] and uploads up to [`LIGHTS_COUNT_MAX`] of them.
//! [`update_camera_uniform`] builds the camera matrix from the sole [`CameraComponent`]; it
//! does nothing when there is no camera and **panics if more than one camera exists**.
//!
//! #### Post-processing
//!
//! Each effect is skipped unless enabled in its resource, and each works on the main target.
//! [`render_exposure_system`] multiplies the image by `2^exposure` into its own texture and
//! copies the result back. [`render_bloom_system`] gathers the pixels above
//! `threshold` (softened over `threshold_softening` with a `smoothstep` on luminance), then for
//! each of [`BLUR_LAYERS_COUNT`] progressively halved layers downscales, blurs horizontally and
//! vertically, and additively applies the layer back onto the main target scaled by
//! `intensity`, ping-ponging between three textures in [`BloomRenderResource`].
//! [`render_color_grading_system`] applies the ACES filmic curve
//! (`shader_color_grading_aces.wgsl`) into its own texture and copies it back. Without color
//! grading the HDR values are written to the surface unmapped.
//!
//! #### Gizmos
//!
//! [`GizmosResource`] keeps a line list per `RenderSpace`; [`render_gizmos`] runs after
//! post-processing, so gizmos are not affected by it. It pops lines from each list in chunks of
//! [`GIZMO_LINES_PER_DRAW_MAX`] through a line-list pipeline backed by storage buffers — which is
//! why gizmos must be re-pushed every frame. World-space gizmos are skipped when there is no
//! camera.
//!
//! #### FFI
//!
//! The GPU-side resources are annotated `todo: support ffi`; only the user-facing settings
//! resources and [`GizmosResource`] are `#[repr(C)]` so far.

mod assets;
mod components;
mod resources;
mod systems;
mod utils;

pub use self::{assets::*, components::*, resources::*, systems::*, utils::*};

use fruits_asset_storage::AssetStorageResource;
use fruits_ecs::{Schedule, WorldBuilderMut};
use fruits_render_core::{StandardMesh, StandardTexture, StandardMaterial};

pub const SYSTEM_GROUP_RENDER: &'static str = "fruits_render";
pub const SYSTEM_GROUP_RENDER_INTERNAL: &'static str = "fruits_render_internal";

pub fn add_render_module_to(mut world: WorldBuilderMut) {
    let mut res = world.data_mut().into_resources_mut();
    res.insert(AssetStorageResource::<StandardMaterial>::new());
    res.insert(AssetStorageResource::<StandardMesh>::new());
    res.insert(AssetStorageResource::<StandardTexture>::new());
    res.insert(GizmosResource::default());
    res.insert(ScreenSpaceResource::default());
    res.insert(BloomResource::default());
    res.insert(ExposureResource::default());
    res.insert(ColorGradingResource::default());

    let mut world_behavior = world.behavior_mut();

    let mut start = world_behavior.get_mut(Schedule::Start);

    start
        .group(SYSTEM_GROUP_RENDER)
        .insert_child_system(create_standard_render_resource)
        .insert_child_system(recreate_main_render_target_resource)
        .insert_child_system(recreate_depth_texture_resource)
        .insert_child_system(recreate_transparent_target_resource)
        .insert_child_system(recreate_exposure_render_resource)
        .insert_child_system(recreate_bloom_render_resource)
        .insert_child_system(recreate_color_grading_render_resource)
        .insert_child_system(create_gizmos_render_resource);

    start
        .order_system(recreate_main_render_target_resource)
        .before_system(recreate_depth_texture_resource)
        .before_system(recreate_transparent_target_resource)
        .before_system(recreate_exposure_render_resource)
        .before_system(recreate_bloom_render_resource)
        .before_system(recreate_color_grading_render_resource)
        .before_system(create_gizmos_render_resource)
        .before_system(create_standard_render_resource);

    let mut update = world_behavior.get_mut(Schedule::Update);

    update
        .group(SYSTEM_GROUP_RENDER)
        .insert_child_group(SYSTEM_GROUP_RENDER_INTERNAL)
        .insert_child_system(render_main_target_to_surface_system);

    update
        .group(SYSTEM_GROUP_RENDER_INTERNAL)
        .insert_child_system(update_camera_uniform)
        .insert_child_system(recreate_main_render_target_resource)
        .insert_child_system(recreate_depth_texture_resource)
        .insert_child_system(recreate_transparent_target_resource)
        .insert_child_system(recreate_exposure_render_resource)
        .insert_child_system(recreate_bloom_render_resource)
        .insert_child_system(recreate_color_grading_render_resource)
        .insert_child_system(clear_main_render_target)
        .insert_child_system(clear_depth)
        .insert_child_system(clear_transparent_target)
        .insert_child_system(update_lights_buffer)
        .insert_child_system(update_global_uniforms)
        .insert_child_system(render_opaque_instanced)
        .insert_child_system(render_opaque_batched)
        .insert_child_system(render_transparent_instanced)
        .insert_child_system(render_transparent_batched)
        .insert_child_system(render_transparent_final_system)
        .insert_child_system(render_exposure_system)
        .insert_child_system(render_bloom_system)
        .insert_child_system(render_color_grading_system)
        .insert_child_system(render_gizmos);

    update
        .order_system(recreate_main_render_target_resource)
        .before_system(recreate_depth_texture_resource)
        .before_system(recreate_transparent_target_resource)
        .before_system(recreate_exposure_render_resource)
        .before_system(recreate_bloom_render_resource)
        .before_system(recreate_color_grading_render_resource)
        .before_system(clear_main_render_target)
        .before_system(clear_depth)
        .before_system(clear_transparent_target)
        .before_system(update_camera_uniform)
        .before_system(update_lights_buffer)
        .before_system(update_global_uniforms)
        .before_system(render_opaque_instanced)
        .before_system(render_opaque_batched)
        .before_system(render_transparent_instanced)
        .before_system(render_transparent_batched)
        .before_system(render_transparent_final_system)
        .before_system(render_exposure_system)
        .before_system(render_bloom_system)
        .before_system(render_color_grading_system)
        .before_system(render_gizmos);

    update.order_group(SYSTEM_GROUP_RENDER_INTERNAL).before_system(render_main_target_to_surface_system);
}
