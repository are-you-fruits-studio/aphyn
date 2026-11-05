# Render features and the RenderPassList — tech design

Status: design ready for implementation (phase 1). Items marked **[my call]** were decided while writing the
document and were not explicitly discussed; review them first.

## Intent

Rendering is hardcoded today: `aphyn_render_ecs` registers a fixed set of systems in the `RenderCamera` schedule that
draw the standard materials, batched UI geometry, gizmos and post-processing. A game cannot add a render stage or a
new kind of material without changing the engine.

The app is restructured from "ECS: systems, resources, entities+components, events" into three parts:

- **systems** (with their existing system resources),
- **world** (resources, events, entities+components),
- **render features**, which turn the world state into GPU work every time a view is rendered.

Materials stop being hardcoded too: a material is a *material template* plus a block of parameters, and templates are
written by the engine or by the game in the same way.

## Goals

- Render features: the most general unit of rendering. A feature can render anything (objects, buffers, textures,
  global or local effects) by adding passes to the frame's `RenderPassList`.
- `RenderPassList`: an array of passes, each with an absolute `u64` order, sorted and executed sequentially.
- Material templates and the engine's `MaterialRenderFeature`, which draws every entity that has a mesh and a material
  through that material's template.
- Creating passes does not allocate (in steady state). All per-frame work reuses state owned by features/templates.
- Features and templates can be defined in the game's dynamic library.
- A view can be rendered 0..n times per frame (game window, editor Scene window, nothing).

## Non-goals (this design)

- A real render *graph*: no automatic resource dependencies, transient/aliased textures or barriers. The name
  `RenderPassList` is deliberate; turning it into a graph with reused GPU resources is a later step.
- More than one camera per rendered view.
- Executing passes in parallel or on another thread.
- Migrating the existing rendering (see "Phases"). Phase 1 builds the framework next to the old path.

## Anti-goals

- Allocating per pass or per frame on the hot path (boxed closures per pass, fresh `Vec`s every frame).
- Systems reaching into features/templates through the ECS.

## Frame flow

1. The game library is called on init. It registers systems and resources into the `World`, and render features and
   material templates into the `RenderRegistry` paired with that world.
2. The `Start` schedule runs.
3. Every frame:
   1. The `Update` schedule runs.
   2. The app calls `RenderRegistry::render_view` for the game camera into its own target, then presents that target
      to the window. Presenting is app behavior, not a render feature: it runs after the `RenderPassList`, using the
      existing `RenderApiNew::draw_to_surface`, so `aphyn_render_core` needs no change for it.
   3. Other code (the editor's Scene window) may call `render_view` on other worlds/targets in the same frame.

### `render_view`

```rust
impl RenderRegistry {
    pub fn render_view(&mut self, world: &mut World, view: CurrentRenderingResource);
}
```

The view is the existing `CurrentRenderingResource`: camera parameters (position, scale-rotation, `Camera`, the
editor's optional extra light) and the color target texture. `render_view`:

1. inserts `CurrentRenderingResource` built from the view,
2. **phase 1 only:** runs the legacy `RenderCamera` schedule, so the old rendering keeps working until migrated
   **[my call]**,
3. clears the `RenderPassList`,
4. calls every feature's `add_passes`, sequentially, in registration order (features mutate the world, so their
   order is defined) **[my call: registration order]**,
5. sorts the list by order with an unstable sort — passes with equal order have **no defined relative order**,
6. runs each pass sequentially on the calling thread,
7. removes `CurrentRenderingResource`.

Passes find the camera and the target through `CurrentRenderingResource`, as the current systems do.

## Registry and storage

`RenderRegistry` stores features and material templates by name. It is **not** an ECS resource: systems never need it.

- `App` holds `World` + `RenderRegistry`.
- The editor's `SimulatedWorldResource` holds the simulated `World` + its own `RenderRegistry`, so the game's
  features render the simulated world and the editor's features render the editor's world.
- `AppInitCtx` gains `render: &mut RenderRegistry`, and `AppInitCtxFfi` a matching raw pointer. As with the other
  fields, the launcher and `aphyn_entry_point!` sides change in lockstep.
- In the editor's Edit mode the simulated world keeps erasing the game's systems after the game library's init (as
  `build_simulated_world` does now), but its `RenderRegistry` keeps **all** the game's render features and material
  templates, so custom materials and effects show in the Scene view. Features are expected not to change the world
  in ways that matter for editing.

```rust
registry.add_feature("bloom", BloomFeature::default());
registry.add_material_template("water", WaterTemplate::default());
```

Registering a name twice replaces nothing and returns an error/`false` **[my call]**.

The engine's `MaterialRenderFeature` is built into the registry and owns the template storage;
`add_material_template` forwards to it **[my call]**.

## Render features

```rust
pub trait RenderFeature: 'static {
    type Pass: 'static; // small feature-defined value, e.g. enum BloomPass { Downsample(u8), Upsample(u8), Composite }

    fn add_passes(&mut self, world: WorldDataMut, passes: &mut RenderPassWriter<Self::Pass>);
    fn run_pass(&mut self, pass: &Self::Pass, ctx: &mut RenderPassCtx);
}

// inside add_passes:
passes.add(RENDER_ORDER_POST_PROCESSING, BloomPass::Composite);
```

- A pass is **not a closure**. It is an order plus a value of the feature's `Pass` type. When the pass runs, the
  feature gets `&mut self` back, so GPU buffers, cached bind groups, gathered entity lists and any other per-frame
  data live in the feature's state and are reused across frames. Passes run sequentially, so handing out `&mut self`
  per pass is sound.
- `add_passes` gets mutable world access: features may update resources and send events.
- `RenderPassCtx` exposes the render API (`RenderApiNew`) and `WorldDataMut` **[my call: mutable, same as features]**.
- GPU resources shared between features (a shadow map sampled by the opaque pass, the HDR color target read by
  post-processing) are **ECS resources**, owned by whichever feature creates them.

### Allocation-free pass list

- Each feature slot keeps a `Vec<Self::Pass>`; the shared `RenderPassList` keeps `(order: u64, slot: u32, index: u32)`
  entries. Both are cleared every `render_view` and keep their capacity, so after warm-up adding and sorting passes
  does not allocate.
- Across the dylib boundary each typed feature is stored erased: an FFI box plus an `extern "C-unwind"` vtable
  (`add_passes`, `run_pass(index)`, `clear_passes`, `drop`), the same technique `TransSerializerRegistry` uses for
  serializers. `RenderPassWriter<P>` is a typed view over the slot's pass vec and the shared list.

### Engine orders

Public constants (names **[my call]**):

| constant                         | order      |
|----------------------------------|------------|
| `RENDER_ORDER_DEPTH_PREPASS`     | 1 000 000  |
| `RENDER_ORDER_SHADOW`            | 3 000 000  |
| `RENDER_ORDER_OPAQUE`            | 5 000 000  |
| `RENDER_ORDER_SKYBOX`            | 7 000 000  |
| `RENDER_ORDER_TRANSPARENT`       | 8 000 000  |
| `RENDER_ORDER_POST_PROCESSING`   | 9 000 000  |

Custom passes pick any order equal to, near or far from these. Stages for UI and gizmos are added in the migration
phase.

## Materials

### Data

```rust
pub struct Material {
    pub template: FfiString, // material template name
    pub data: MaterialData,
}
pub struct MaterialComponent { pub material: AssetHandle<Material> }
pub struct MeshComponent { pub mesh: AssetHandle<StandardMesh> }
```

Materials are stored in `AssetStorageResource<Material>`. The new component and asset names must not collide with
the legacy `Standard*` ones while both exist **[my call: names]**.

### `MaterialData`

Material parameters are not a typed Rust struct per template. They are an untyped, name-keyed map, already started in
`crates/render/src/assets/material.rs`:

```rust
enum MaterialDataValue { F32(f32), Vec4(Vec4<f32>), Texture(AssetHandle<StandardTexture>) }
struct MaterialDataEntry { value: MaterialDataValue, snapshot: u64 }
struct MaterialData(FfiIndexMap<FfiString, MaterialDataEntry>);
// typed accessors: get_f32 / get_f32_mut, get_vec4 / get_vec4_mut, get_texture / get_texture_mut
```

- **FFI-safe:** `MaterialDataValue` is `#[repr(C, u8)]`, `MaterialDataEntry` `#[repr(C)]`, `MaterialData`
  `#[repr(transparent)]`; all of them and `Material` are `pub` and can cross the dylib boundary.
- **Serializable:** `MaterialData` implements `Serializable` as a map of name → value, so `Material` is plain
  serializable data. No type erasure, no type names on disk, no serializer registration per template. `snapshot` is
  not serialized; loaded entries start at snapshot `1` **[my call]**.
- **Change detection:** every mutable access to an entry (`get_mut` and the typed `*_mut` accessors) bumps its
  `snapshot` (wrapping, skipping `0`, as the existing `// todo` says). Templates remember the last snapshot they
  uploaded per material entry and update only what changed. Deserializing into an existing `MaterialData` (inspector
  edits) must bump the snapshots of the entries it writes **[my call]**.
- **Template defaults:** each template provides `default_data()`, a `MaterialData` filled with *its own* defaults
  (e.g. water starts blue with `wave_speed = 0.3`), not the absolute zero/white defaults. It is used to create a new
  material for a template and by the editor to show the template's fields.
- **Reading at render time:** a template reads entries by name with the typed accessors. An entry that is missing or
  has the wrong value type falls back to the value from the template's `default_data()`; extra entries are ignored
  **[my call]**. An unknown template name skips the material and logs once per material.
- New value kinds (e.g. `Vec2`, `Mat4`, `U32`, cube textures) are added to `MaterialDataValue` when templates need them.

### Saving and loading

- On disk: `{ "template": "water", "data": { "color": ..., "wave_speed": ..., "normal_tex": ... } }`. Asset loading
  never needs the template storage.
- `Material` reuses the existing `"material"` asset type (`AssetType::Material` in `aphyn_asset_loading`), which
  now loads and saves `Material` instead of the legacy `StandardMaterial`. No backwards compatibility: existing
  material files (e.g. `crates/editor/workspace/prefabs/some_material.json`) and on-disk `StandardMaterial`s stop
  loading; legacy materials can still be created in code until the migration removes them.
- The editor inspector edits `Material.data` as ordinary serializable data. Creating a new material in the editor
  picks a template name and fills `data` from that template's `default_data()` (the editor reaches the registry
  through `SimulatedWorldResource`, which owns it).

### Material templates

A template depends on (possibly many) shaders and uses them for different stages. It owns the per-material GPU data:
it builds uniform buffers, textures and bind groups from a material's `MaterialData`, caches them per material, and
updates them when entry snapshots change. `MaterialRenderFeature` never interprets material data.

If the rendering architecture needs a depth prepass, a shadow pass and an opaque pass, the template schedules all of
them.

```rust
pub trait MaterialTemplate: 'static {
    type Pass: 'static;

    fn default_data(&self) -> MaterialData;
    fn add_passes(&mut self, world: WorldDataMut, batches: &[MaterialBatch], passes: &mut RenderPassWriter<Self::Pass>);
    fn run_pass(&mut self, pass: &Self::Pass, ctx: &mut RenderPassCtx);
}
```

Templates are stored and erased the same way as features and share the same `RenderPassList`.

### `MaterialRenderFeature`

An engine render feature that renders entities with `MeshComponent` + `MaterialComponent` + `GlobalTransform`:

1. gathers them into a reused vec,
2. resolves each material's template; skips unresolved ones,
3. sorts by template → material → mesh,
4. writes all instance transforms into one reused, growable GPU instance buffer, stored as an ECS resource
   (`MaterialInstancesResource`) so templates' `run_pass` can bind it through `RenderPassCtx`'s world access,
5. calls each template's `add_passes` with that template's contiguous run of `MaterialBatch`es.

A `MaterialBatch` names the material handle, the mesh handle and the instance range in `MaterialInstancesResource`.
Exactly what else it carries (e.g. per-entity data for transparent back-to-front sorting) is left to the builder.

## Phases

1. **Framework (this design):** `RenderRegistry`, `RenderPassList`, the traits and their FFI erasure, `render_view`,
   `Material`/`MaterialComponent`/`MeshComponent`, `MaterialRenderFeature`, app + `AppInitCtx` + editor wiring,
   loading/saving `Material` through the `"material"` asset type, and editing its data in the inspector.
   Prove it with one simple unlit template and one fullscreen feature in an example **[my call]**. Legacy rendering
   keeps running through the `RenderCamera` schedule inside `render_view`. `MaterialData` and `Material` live in
   `aphyn_render` next to the legacy material. Legacy and new output do **not** have to
   compose correctly (separate depth targets, new passes drawn after legacy post-processing): the intermediate state is
   only for verification, never shipped. The example uses only the new path and owns its own depth target.
2. **Migration (later, own design pass):** port standard lit/unlit/opaque/transparent shaders and the UI variants to
   templates, `BatchedMeshComponent` geometry, gizmos and exposure/bloom/color grading to features; add UI and gizmo
   orders; delete the legacy systems, `StandardMaterial*` and the `RenderCamera` schedule.
3. **Later:** turn `RenderPassList` into a graph with reused GPU resources.

## Left to the builder

- Crate placement of the framework (a new crate vs. a module of `aphyn_render_ecs`); `aphyn_app` must depend on it.
- Instance buffer growth policy and data layout.
- Exact `RenderPassCtx` / `RenderView` field sets beyond what is listed.
