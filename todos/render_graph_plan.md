# Render Features and RenderPassList Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build phase 1 of the new rendering framework — `RenderRegistry`, `RenderPassList`, render features, material templates, `Material`/`MaterialData` assets — wired into the app, the dynamic launcher and the editor, next to the legacy rendering.

**Architecture:** A new crate `aphyn_render_features` owns the registry, the pass list, the feature/template traits with their FFI erasure, the material components and the built-in material step. `Material`/`MaterialData` live in `aphyn_render`. `RenderRegistry::render_view` replaces the direct `Schedule::RenderCamera` calls in `aphyn_app` and the editor, and still runs that schedule first so legacy rendering keeps working.

**Tech Stack:** Rust 2024, wgpu via `aphyn_render_core` (`RenderApi`), `aphyn_ffi`, `aphyn_serialization`, `aphyn_ecs`.

**Spec:** `todos/render_graph.md` (read it first; this plan does not repeat its reasoning).

## Global Constraints

- No git writes. Where a task ends, leave changes in the working tree; the user commits.
- Comment policy (CLAUDE.md): only crate-root `//!` docs, `// todo` markers, rare one-line notes. No `///` docs.
- Run `cargo fmt -p <crate>` only for crates you touched, never workspace-wide.
- Everything stored in the registry or crossing the dylib boundary uses `aphyn_ffi` types (`FfiVec`, `FfiString`, `FfiIndexMap`, `FfiDroppable`, `extern "C-unwind"` vtables). Follow `TransSerializerFfi` in `crates/serialization/src/serialization_registry.rs`.
- `aphyn_modules::add_defult_modules_to` keeps its misspelling.
- Engine order constants exactly: `RENDER_ORDER_DEPTH_PREPASS = 1_000_000`, `RENDER_ORDER_SHADOW = 3_000_000`, `RENDER_ORDER_OPAQUE = 5_000_000`, `RENDER_ORDER_SKYBOX = 7_000_000`, `RENDER_ORDER_TRANSPARENT = 8_000_000`, `RENDER_ORDER_POST_PROCESSING = 9_000_000`.
- Passes with equal order have no defined relative order (unstable sort).
- After warm-up, collecting and sorting passes must not allocate.
- Changing crate behavior or public API means updating that crate's `//!` docs (style per `AGENTS.md`).
- The render-core type is `aphyn_render_core::RenderApi` (crate docs still say `RenderApiNew`; do not rename anything).

## Review Focus

1. A `Material` whose `template` names no registered template — skipped, logged once per material, other materials still draw. (Task 4, `unknown_template_is_skipped_and_logged_once`.)
2. An entity whose mesh or material handle does not resolve in its `AssetStorageResource` — skipped, no panic. (Task 4, `unresolved_handles_are_skipped`.)
3. A frame with no features, no templates and no material entities — `render_view` runs, draws nothing, no panic. (Task 2, `empty_registry_collects_nothing`.)
4. A `MaterialData` entry missing or of the wrong kind — the template reads the value from its `default_data()`. (Task 1, `typed_get_or_default_falls_back`.)
5. Entity count growing past the instance buffer capacity — buffer grows, every entity is drawn. (Task 4, `instance_capacity_grows_to_fit`.)

---

### Task 1: `MaterialData` and `Material` in `aphyn_render`

**Files:**
- Modify: `crates/render/src/assets/material.rs:211-273` (the draft `MaterialData*` types)
- Modify: `crates/render/src/lib.rs` (exports and `//!` docs)

**Interfaces:**
- Produces (all `pub`, exported from `aphyn_render`):
  - `#[repr(C, u8)] #[derive(Clone, Debug, PartialEq, Serializable)] pub enum MaterialDataValue { F32(f32), Vec4(Vec4<f32>), Texture(AssetHandle<StandardTexture>) }`
  - `#[repr(C)] #[derive(Clone, Debug)] pub struct MaterialDataEntry` with `new(value) -> Self`, `get(&self) -> &MaterialDataValue`, `get_mut(&mut self) -> &mut MaterialDataValue` (bumps snapshot), `snapshot(&self) -> u64`.
  - `#[repr(transparent)] #[derive(Clone, Debug, Default)] pub struct MaterialData(FfiIndexMap<FfiString, MaterialDataEntry>)` with `new`, `insert(&mut self, name: &str, value: MaterialDataValue)`, `entry(&self, name) -> Option<&MaterialDataEntry>`, typed accessors `get_f32 / get_f32_mut`, `get_vec4 / get_vec4_mut`, `get_texture / get_texture_mut` (rename the draft's `get_g32_mut`), and `get_f32_or(&self, name, default: &MaterialData) -> f32` plus `get_vec4_or`, `get_texture_or` (fall back to `default`, then to `0.0` / `Vec4::ZERO`-equivalent / `AssetHandle::EMPTY`).
  - `#[repr(C)] #[derive(Clone, Debug, Default, Serializable)] pub struct Material { pub template: FfiString, pub data: MaterialData }`
- `MaterialDataEntry` implements `Serializable` by hand: serializes as its value only; `deserialize` writes the value and bumps the snapshot. `MaterialData` serializes as the map (derive or hand-written delegating to the inner `FfiIndexMap`). Serialization must work through the same ctx states `StandardMaterialAssetMetadata` uses, so texture handles go through `AssetHandleLinkTransSerializer` in asset loading.
- Snapshot bump: private `fn next_snapshot(s: u64) -> u64` — wrapping add, skipping `0`. Loaded/new entries start at `1`. Remove the existing `// todo: wrapping add and avoid 0`.

- [ ] **Step 1: Write failing tests** (`#[cfg(test)] mod tests` in `material.rs`; use a `TransSerializerRegistry` with `StandardSerializer`s for `f32`, `Vec4<f32>`, `FfiString`, and the new types — no textures in these tests):
  - `roundtrip_keeps_values`: data `{ "speed": F32(0.3), "color": Vec4(1,0,0,1) }` serialized and deserialized into `MaterialData::default()` gives equal values in the same order; serialized JSON for the entry `speed` is the bare value, no `snapshot` key.
  - `get_mut_bumps_snapshot`: new entry snapshot `1`; after `get_f32_mut("speed")` it is `2`; after `get_f32("speed")` still `2`.
  - `deserialize_bumps_snapshot`: deserializing `speed` at path `"speed"` into existing data bumps that entry's snapshot only.
  - `next_snapshot_skips_zero`: `next_snapshot(u64::MAX) == 1`, `next_snapshot(1) == 2`.
  - `typed_get_or_default_falls_back`: data lacks `speed` and has `color` as `F32`; defaults have `speed: 0.3`, `color: Vec4(0,0,1,1)`; `get_f32_or("speed", &defaults) == 0.3`, `get_vec4_or("color", &defaults) == Vec4(0,0,1,1)`.
  - `material_roundtrip`: `Material { template: "water", data }` roundtrips; JSON has keys `template` and `data`.
- [ ] **Step 2: Run** `cargo test -p aphyn_render material` — expected FAIL (types private / missing).
- [ ] **Step 3: Implement** the interfaces above; export from `crates/render/src/lib.rs`; add a short `//!` section "Material data" (the map, snapshots, defaults) under *How to use*, and the snapshot/serialization rules under *How to maintain*.
- [ ] **Step 4: Run** `cargo test -p aphyn_render` — expected PASS (including existing doctests). `cargo fmt -p aphyn_render`.

---

### Task 2: `aphyn_render_features` crate — registry, pass list, features

**Files:**
- Create: `crates/render_features/Cargo.toml` (deps: `aphyn_ecs`, `aphyn_ffi`, `aphyn_render`, `aphyn_render_core`, `aphyn_asset_storage`, `aphyn_transform`)
- Create: `crates/render_features/src/lib.rs` (`//!` docs + re-exports), `src/orders.rs`, `src/pass_list.rs`, `src/feature.rs`, `src/registry.rs`
- Modify: root `Cargo.toml` (workspace member + `aphyn` dependency), `src/lib.rs` (`pub use aphyn_render_features::*;`)

**Interfaces:**
- Produces:
  - the six `pub const RENDER_ORDER_*: u64` (Global Constraints).
  - `pub trait RenderFeature: Send + Sync + 'static { type Pass: Send + Sync + 'static; fn add_passes(&mut self, world: WorldDataMut, passes: &mut RenderPassWriter<Self::Pass>); fn run_pass(&mut self, pass: &Self::Pass, ctx: &mut RenderPassCtx); }`
  - `#[repr(C)] pub struct RenderPassEntry { pub order: u64, pub slot: u32, pub index: u32 }`
  - `pub struct RenderPassWriter<'a, P>` with `pub fn add(&mut self, order: u64, pass: P)` — pushes `pass` into the slot's own `FfiVec<P>` and an entry into the shared list.
  - `pub struct RenderPassCtx<'a> { pub render_api: &'a RenderApiResource, pub world: WorldDataMut<'a> }`
  - `pub struct RenderRegistry` (`Send + Sync`, `Default`) with:
    - `pub fn add_feature<F: RenderFeature>(&mut self, name: &str, feature: F) -> bool` — `false` and no change if the name is taken by any feature or template.
    - `pub fn render_view(&mut self, world: &mut World, view: CurrentRenderingResource)` — the spec's 7 steps. The spec's `RenderView` is `CurrentRenderingResource` itself (same fields; no new type). Gets a clone of `RenderApiResource` from the world before running passes; if it is missing, collects nothing and returns after the legacy schedule.
    - `pub(crate) fn collect_passes(&mut self, world: &mut World)` — clear, call every slot's `add_passes` in registration order, unstable-sort the list by `order`.
    - `pub(crate) fn pass_entries(&self) -> &[RenderPassEntry]` (tests).
- Storage: one `FfiVec` of erased slots; each slot = boxed typed state (`feature` + `FfiVec<Pass>`) as `FfiDroppable` + `&'static` `extern "C-unwind"` vtable (`add_passes`, `run_pass(index, ctx)`, `clear_passes`). A `RenderPassEntry.slot` indexes that vec; templates (Task 4) share the same slot space. Names in an `FfiIndexMap<FfiString, u32>`.

- [ ] **Step 1: Write failing tests** (`#[cfg(test)] mod tests` in `registry.rs`; a test feature that pushes fixed orders, and a world built with `WorldBuilder::new().build()`):
  - `passes_are_sorted_by_order`: feature A adds orders `[9_000_000, 1_000_000]`, feature B adds `[5_000_000]` → entries' orders `[1_000_000, 5_000_000, 9_000_000]`, and each entry's `slot`/`index` points back to the pass value that was added (assert via a test-only accessor or by checking A's slot id and indices).
  - `features_run_in_registration_order`: two features each push their name into a shared `Vec<&str>` resource in `add_passes` → `["a", "b"]`.
  - `duplicate_name_is_rejected`: second `add_feature("a", ..)` returns `false`; only one slot exists.
  - `collecting_twice_does_not_reallocate`: run `collect_passes` twice with the same pass count; the shared list's and the slot's pass vec data pointers and capacities are unchanged after the second call.
  - `empty_registry_collects_nothing`: `RenderRegistry::default()` → `collect_passes` → no entries, no panic.
- [ ] **Step 2: Run** `cargo test -p aphyn_render_features` — expected FAIL (crate items missing).
- [ ] **Step 3: Implement** the interfaces. `render_view` order: insert `CurrentRenderingResource` → `world.execute_iteration(Schedule::RenderCamera)` (`// todo: remove after migration`) → `collect_passes` → run each entry via its slot's vtable with a fresh `RenderPassCtx` → remove the resource. Write the crate `//!` docs (*How to use*: registering a feature, adding passes, order constants, sharing GPU resources through ECS resources; *How to maintain*: slot erasure, allocation-free invariant, the legacy schedule call, dylib drop-order requirement: a registry holding slots from a library must be dropped before that library is unloaded).
- [ ] **Step 4: Run** `cargo test -p aphyn_render_features` and `cargo build` — expected PASS. `cargo fmt -p aphyn_render_features`.

---

### Task 3: App and launcher wiring

**Files:**
- Modify: `crates/app/Cargo.toml`, `crates/app/src/app.rs`, `crates/app/src/init_ctx.rs`, `crates/app/src/event_loop_handler.rs:25-35,236-275`, `crates/app/src/lib.rs` (`//!`)
- Modify: `crates/app_launcher/src/lib.rs` (`//!` only if it describes `AppInitCtxFfi` fields)

**Interfaces:**
- Consumes: `RenderRegistry`, `render_view` (Task 2).
- Produces:
  - `App` gains a `render: RenderRegistry` field and `pub fn render_mut(&mut self) -> &mut RenderRegistry`; `init_ctx_mut` fills the new ctx field.
  - `AppInitCtx<'a> { pub world, pub window, pub render: &'a mut RenderRegistry }`; `AppInitCtxFfi` gains `pub render_mut: *mut RenderRegistry`, set in `into_ffi` and read in `init_app`.
  - `EventLoopHandlerState::Polling` holds the registry; `core_system_render` calls `render.render_view(world, CurrentRenderingResource { .. })` instead of inserting/removing the resource and running `RenderCamera` itself; clearing the target and `render_to_surface` stay as they are.

- [ ] **Step 1: Implement** the changes (no automated test: needs a window).
- [ ] **Step 2: Run** `cargo build` — expected success across the workspace (fix every `AppInitCtx { .. }` construction site, including the editor's `build_simulated_world`; for the editor pass a temporary local `RenderRegistry` — Task 5 replaces it).
- [ ] **Step 3: Run** `cargo run -p aphyn_example_cubes` — expected: cubes render exactly as before. Update `crates/app/src/lib.rs` `//!` (ctx field, presenting after `render_view`, lockstep note). `cargo fmt -p aphyn_app`.

---

### Task 4: Material templates and the built-in material step

**Files:**
- Create: `crates/render_features/src/material.rs` (template trait, components, batches, gather/sort, instances resource)
- Modify: `crates/render_features/src/registry.rs`, `src/lib.rs` (+ `//!`)
- Create: `pub fn add_render_features_module_to(world: WorldBuilderMut)` in `src/lib.rs` — inserts `AssetStorageResource<Material>`
- Modify: `crates/modules/Cargo.toml` (depend on `aphyn_render_features`), `crates/modules/src/lib.rs` — call it in `add_defult_modules_to`; register `Material`, `MaterialData`, `MaterialDataEntry`, `MaterialDataValue`, `MaterialComponent`, `MeshComponent` in `register_common_transserializers`

**Interfaces:**
- Consumes: Task 1 types; Task 2 slots/writer/ctx.
- Produces:
  - `pub trait MaterialTemplate: Send + Sync + 'static { type Pass: Send + Sync + 'static; fn default_data(&self) -> MaterialData; fn add_passes(&mut self, world: WorldDataMut, batches: &[MaterialBatch], passes: &mut RenderPassWriter<Self::Pass>); fn run_pass(&mut self, pass: &Self::Pass, ctx: &mut RenderPassCtx); }`
  - `#[repr(C)] #[derive(Component, Serializable, Clone, Default)] pub struct MaterialComponent { pub material: AssetHandle<Material> }` and `MeshComponent { pub mesh: AssetHandle<StandardMesh> }` (follow `StandardMeshComponent`'s derives).
  - `#[repr(C)] #[derive(Clone)] pub struct MaterialBatch { pub material: AssetHandle<Material>, pub mesh: AssetHandle<StandardMesh>, pub instance_start: u32, pub instance_count: u32 }`
  - `#[derive(Resource)] pub struct MaterialInstancesResource { pub buffer: StandardInstanceBuffer }` — created lazily on the first frame with instances, through `RenderApiResource`.
  - `fn next_instance_capacity(current: u32, needed: u32) -> u32` — `current` if enough, else max(`needed`, `current * 2`, 64).
  - `RenderRegistry::add_material_template<T: MaterialTemplate>(&mut self, name: &str, template: T) -> bool` (same name rules as features).
  - `RenderRegistry::create_material(&self, template: &str) -> Option<Material>` — `Material { template, data: default_data() }`.
  - `RenderRegistry::template_default_data(&self, template: &str) -> Option<MaterialData>` (for the editor).
- `collect_passes` now first runs the material step, then features in registration order **[spec's my-call: material step owned by the registry]**:
  1. query `(&MeshComponent, &MaterialComponent, &GlobalTransform)` into a reused `FfiVec` of `(template_slot, material, mesh, local_to_world)`; skip entities whose material or mesh handle is unresolved, and materials whose template is unknown (log `unknown material template '<name>'` once per material handle, tracked in a reused set);
  2. sort by `(template_slot, material, mesh)`;
  3. group into `MaterialBatch`es, grow the instance buffer with `next_instance_capacity`, write all transforms with `write_instances_with`;
  4. call each template slot's `add_passes` with its contiguous batch slice.
- The pure part (steps 2–3 minus GPU writes) is `fn build_batches(items: &mut [GatheredInstance], batches: &mut FfiVec<MaterialBatch>)` so it is testable without a GPU.

- [ ] **Step 1: Write failing tests** (`#[cfg(test)]` in `material.rs`):
  - `batches_group_by_template_material_mesh`: items `(t1,m2,meshA)`,`(t0,m1,meshA)`,`(t1,m2,meshA)`,`(t0,m1,meshB)` → batches `[(m1,meshA,0,1),(m1,meshB,1,1),(m2,meshA,2,2)]`.
  - `instance_capacity_grows_to_fit`: `next_instance_capacity(0, 3) == 64`, `(64, 64) == 64`, `(64, 65) == 128`, `(64, 1000) == 1000`.
  - `unresolved_handles_are_skipped`: world with one entity whose `MaterialComponent` handle is not in storage and one valid entity (template registered, no `RenderApiResource` in world) → gathered items contain only the valid entity; no panic.
  - `unknown_template_is_skipped_and_logged_once`: material with template `"missing"`; collect twice → item skipped both times; the logged-set contains the handle once (assert on the set, not stdout).
  - `create_material_uses_template_defaults`: template with `default_data() = { speed: 0.3 }` → `create_material("t").unwrap().data.get_f32("speed") == Some(&0.3)`; unknown name → `None`.
  - `templates_and_features_share_names`: `add_feature("x")` then `add_material_template("x")` → `false`.
- [ ] **Step 2: Run** `cargo test -p aphyn_render_features` — expected FAIL.
- [ ] **Step 3: Implement.** Without `RenderApiResource` in the world, skip only the GPU write (so the tests above can gather). Update `//!` docs: writing a template, `default_data`, reading data with `get_*_or`, snapshot-driven GPU updates, `MaterialInstancesResource`.
- [ ] **Step 4: Run** `cargo test -p aphyn_render_features` and `cargo build` — expected PASS. `cargo fmt -p aphyn_render_features -p aphyn_modules`.

---

### Task 5: Editor wiring

**Files:**
- Modify: `crates/editor/src/features/world_preload.rs:28-50,116-200`, `crates/editor/src/features/scene_window.rs:217-316`, `crates/editor/src/features/game_window.rs:~99`

**Interfaces:**
- Consumes: `RenderRegistry`, `render_view`, `add_render_features_module_to`.
- Produces:
  - `SimulatedWorld` gains `pub render: RenderRegistry`, declared **after `world` and before `library`** so it drops before the library unloads; `SimulatedWorld::new(world, render, library, mode)`.
  - `build_simulated_world(..) -> (World, RenderRegistry)`: passes `render: &mut registry` in `AppInitCtx`. In Edit mode the game's systems are still cleared, the registry is kept as is; add `add_render_features_module_to` next to the other Edit-mode modules.
  - `render_simulated_view(.., world: &mut World, render: &mut RenderRegistry, camera)` calls `render.render_view(world, CurrentRenderingResource { .. })` instead of the manual insert/schedule/remove.

- [ ] **Step 1: Implement.**
- [ ] **Step 2: Run** `cargo build -p aphyn_editor` — expected success.
- [ ] **Step 3: Run** `cargo run -p aphyn_editor -- edit crates/editor/workspace` — expected: Scene and Game windows render as before; switching Play ↔ Edit (rebuilds the simulated world and reloads the library) does not crash. `cargo fmt -p aphyn_editor`.

---

### Task 6: `"material"` asset type loads and saves `Material`

**Files:**
- Modify: `crates/asset_loading/src/material.rs`, `src/lib.rs:242,439` (+ `//!`), `src/serialized_entities.rs:~40`, `src/serializers/load_single.rs`, `src/serializers/load_transitively.rs`, `src/serializers/save_load.rs`
- Modify: `crates/editor/src/features/asset_serialization.rs` (`to_serialized`, `get_asset_type`), `crates/editor/src/features/inspector_window/utils/serialization.rs` (both `AssetType::Material` arms)

**Interfaces:**
- Consumes: `Material` (Task 1), `AssetStorageResource<Material>` (Task 4).
- Produces: every `AssetType::Material` code path uses `Material` / `AssetStorageResource<Material>` instead of `StandardMaterial`. `MaterialLoader::load_from_deserialized(Material) -> Option<Material>` needs no GPU. Saving: `DirectAssetSaveTransSerializer { assets: materials, extractor: |_, m| m.clone() }`. Keep `AssetHandleLinkTransSerializer` for both `StandardMaterial` and `Material` storages (legacy in-code materials still referenced by handles). No backwards compatibility: `crates/editor/workspace/prefabs/some_material.json` will fail to load; leave the file untouched and mention it in the hand-off.

- [ ] **Step 1: Write a failing doctest** in the `aphyn_asset_loading` `//!` docs (or a `#[cfg(test)]` test if the crate has no headless doctest setup): a `Material { template: "unlit_color", data: { color: Vec4(1,0,0,1) } }` saved through `save_load_with_asset_serializers` and loaded back through the material loader equals the original; the JSON is `{ "template": "unlit_color", "data": { "color": ... } }`.
- [ ] **Step 2: Run** `cargo test -p aphyn_asset_loading` — expected FAIL.
- [ ] **Step 3: Implement** the swap in every file listed.
- [ ] **Step 4: Run** `cargo test -p aphyn_asset_loading` and `cargo build` — expected PASS. Manual: in the editor, create a material file `{ "asset_type": "material", "template": "unlit_color", "data": { "color": ... } }` in the workspace assets, select it, change `color` in the inspector — the file on disk updates. `cargo fmt` for touched crates.

---

### Task 7: Example — unlit template and fullscreen feature

**Files:**
- Create: `crates/example/material/Cargo.toml` (`aphyn_example_material`, depends on `aphyn`), `src/main.rs`, `src/unlit_color.wgsl`, `src/vignette.wgsl`
- Modify: root `Cargo.toml` (member); `CLAUDE.md` example list in Commands; `CLAUDE.md` crate layering (add `aphyn_render_features` to subsystem crates)

**Interfaces:**
- Consumes: everything above.
- Produces:
  - `UnlitColorTemplate` registered as `"unlit_color"`: `default_data() = { color: Vec4(1,1,1,1) }`; `Pass = ()`; one pass at `RENDER_ORDER_OPAQUE`; draws every batch with `draw_indexed` using the mesh's buffers, `MaterialInstancesResource.buffer` for instances, a template-owned camera uniform (`CurrentRenderingResource::world_to_clip()`), a per-material uniform updated when the `color` entry's snapshot changes, and a template-owned depth texture sized to the target (recreated on size change).
  - `VignetteFeature` registered as `"vignette"`: one fullscreen pass at `RENDER_ORDER_POST_PROCESSING`, reads `CurrentRenderingResource.target` via a copy it owns and draws back into the target with darkened corners.
  - Scene: camera + ~10 cubes with `MeshComponent` + `MaterialComponent` (two materials created via `registry.create_material("unlit_color")`, one with `color` changed), rotating in an `Update` system; one system changes a material's color every second to exercise snapshots.

- [ ] **Step 1: Implement.**
- [ ] **Step 2: Run** `cargo run -p aphyn_example_material` — expected: rotating cubes in two flat colors, one blinking color, vignette darkening the corners, no validation errors in the console.
- [ ] **Step 3: Check** `cargo build --release -p aphyn_editor` and `cargo test -p aphyn_render -p aphyn_render_features -p aphyn_asset_loading` pass. `cargo fmt -p aphyn_example_material`.
