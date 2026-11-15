---
title: Architecture
description: How the Aphyn crates fit together, what runs each frame, and the two ways to launch a game.
---

# Architecture

Aphyn is one Cargo workspace of small crates. Games, examples and the editor depend on a single facade crate, `aphyn`, and bring everything into scope with `use aphyn::*;`.

## Crate layers

```text
 game / examples / editor
            │
          aphyn                      facade: re-exports every engine crate
            │
 aphyn_app · aphyn_app_launcher      window, frame loop, input; static and dynamic launch
            │
 aphyn_modules                       registers the default subsystems in one call
            │
 subsystems                          collision · physics · transform · ui · render · audio · assets · prefabs
            │
 aphyn_ecs                           entities, components, resources, events, systems
            │
 aphyn_ffi · aphyn_serialization · aphyn_math · aphyn_utils
```

| Layer | Crates | Role |
|---|---|---|
| Foundation | `aphyn_ffi`, `aphyn_utils`, `aphyn_math`, `aphyn_serialization`, `aphyn_reflection` | FFI-safe standard types, containers and helpers, linear algebra, serialization and reflection. |
| Core | `aphyn_ecs` | The world: entities, components, resources and events, plus systems scheduled by the data they touch. |
| Subsystems | `aphyn_collision`, `aphyn_physics`, `aphyn_transform`, `aphyn_ui`, `aphyn_render`, `aphyn_render_features`, `aphyn_audio`, `aphyn_asset_storage`, `aphyn_asset_loading`, `aphyn_prefab` | Each adds its components, resources and systems to a world. |
| Wiring | `aphyn_modules` | `add_defult_modules_to` registers every subsystem and the default serializers, and orders them. |
| Runtime | `aphyn_app`, `aphyn_app_launcher`, `aphyn_entry_point`, `aphyn_window` | Window, frame loop and input; launching a game statically or from a dynamic library. |
| Tools | `aphyn_editor` | The editor and the `aphyn build` command, itself an Aphyn app. |

Every crate's page in the [API Reference](pathname:///api-reference/aphyn/index.html) opens with a *How to use* section.

## FFI-safe data

A game can run as a dynamic library loaded by a separate launcher (see [below](#two-ways-to-launch)), so world data crosses a binary boundary. Types stored in the world must therefore have a stable layout. `aphyn_ffi` provides replacements for the standard types that do not: `FfiVec`, `FfiString`, `FfiBox`, `FfiOption`, hash maps, closures and type-erased values. Prefer them in components, resources and public APIs that may cross that boundary.

## Modules

A subsystem crate exposes an `add_<name>_module_to` function that inserts its resources and registers its systems:

| Crate | Function |
|---|---|
| `aphyn_collision` | `add_collision_module_to` |
| `aphyn_physics` | `add_physics_module_to` |
| `aphyn_transform` | `add_transform_module_to` (takes only data and behavior) |
| `aphyn_ui` | `add_ui_module_to` |
| `aphyn_render_features` | `add_render_features_module_to` |
| `aphyn_audio` | `add_audio_module_to` |
| `aphyn_asset_loading` | `add_asset_module_to` |

`add_defult_modules_to(data, behavior, render)` calls all of them, inserts a `SerializersResource` with the engine's common serializers, and orders the groups that depend on each other:

- in `Update`: collision, then transform, then render;
- in `Start`: render before asset loading, because asset files may reference the built-in shaders and material templates.

Each subsystem registers its systems under a group name, exported as a constant, so your systems can be ordered against a whole subsystem:

| Constant | Group |
|---|---|
| `SYSTEM_GROUP_COLLISION` | `aphyn_collision` |
| `SYSTEM_GROUP_PHYSICS` | `aphyn_physics` |
| `SYSTEM_GROUP_TRANSFORM` | `aphyn_transform` |
| `SYSTEM_GROUP_UI` | `aphyn_ui` |
| `SYSTEM_GROUP_RENDER` | `aphyn_render` |
| `SYSTEM_GROUP_AUDIO` | `aphyn_audio` |
| `SYSTEM_GROUP_ASSETS` | `aphyn_assets` |

```rust
use aphyn::*;

fn move_player() {}

fn register(mut behavior: WorldBehaviorBuilderMut) {
    let mut update = behavior.get_mut(Schedule::Update);
    update.insert_system(move_player);
    let _ = update.order_system(move_player).before_group(SYSTEM_GROUP_TRANSFORM);
}
```

## A frame

`App::run` opens the window and drives the world through three schedules:

1. **`Start`** runs once, before the first frame.
2. Each frame, **`FixedUpdate`** runs zero or more times: one pass per whole fixed time step of elapsed time, scaled by `TimeResource::time_scale`. Physics runs here.
3. Then **`Update`** runs exactly once.
4. Rendering is not a schedule. After `Update`, the app renders the view of the first entity with a `GlobalTransform` and a `CameraComponent` through the render features of its `RenderFeatureRegistry`: lights, materials, gizmos and post-processing. With no camera, nothing is drawn.

`TimeResource` describes both clocks: `delta_time` for the frame, `fixed_delta_time` for the current fixed step. Systems may change `time_scale` and `fixed_time_step`; the change applies from the next frame or step.

## Two ways to launch

**Static.** The game is compiled into the executable. Build an `App`, call `add_defult_modules_to`, register systems and call `app.run()`, as in [Quick start](../getting-started.md#your-first-app). `launch_app_statically(|ctx| ...)` does the same with less code:

```rust
use aphyn::*;

fn main() {
    launch_app_statically(|ctx: AppInitCtx| {
        ctx.window.title = "My game".into();
        ctx.window.size = [1280, 720];
    });
}
```

**Dynamic.** The game is a `cdylib` named `lib_app`, and a thin launcher calls `launch_app_dynamically()`. The launcher loads the library from next to its own executable, resolves the C symbol `aphyn_entry_point` generated by `aphyn_entry_point!(setup)`, and passes the world to `setup`. This is how `aphyn build` ships games; see [Build a game](../build-a-game.md).

Both sides of a dynamic launch must be built from the same engine version. The symbol name, its ABI and the layout of the data passed through it are checked only at run time.

## Next

- [ECS](./ecs.md): what a world holds and how systems are scheduled.
- [API Reference](pathname:///api-reference/aphyn/index.html): every crate, starting with its *How to use* section.
