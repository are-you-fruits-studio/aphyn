---
title: Build a game
description: Ship a game as a launcher, a game library and its assets with aphyn build.
---

import AphynDependency from '@site/src/components/AphynDependency';

# Build a game

A shipped game is a folder with three parts: a small **launcher** executable, your game compiled as a dynamic library named **`aphynapp`**, and the **`assets/`** it loads. `aphyn build` produces that folder from a project.

## Project layout

```text
my_game/
├── assets/            # optional; copied into the build
├── scripts/           # your game: one Cargo package with two targets
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs     # the game library (cdylib)
│       └── main.rs    # the launcher binary
└── builds/            # created by `aphyn build`
```

## The game crate

`scripts/Cargo.toml` builds your game as a `cdylib` named `aphynapp` and a binary named `launcher`. `aphyn build` and the launcher look for these names: without `name = "aphynapp"` under `[lib]`, Cargo names the library after the package and the launcher does not find it.

<AphynDependency
  title="scripts/Cargo.toml"
  manifestHead={`[package]
name = "lib_app"
version = "0.1.0"
edition = "2024"

[lib]
name = "aphynapp"
crate-type = ["cdylib"]

[[bin]]
name = "launcher"
path = "src/main.rs"

`}
/>

Use the same tag as the `aphyn` binary you build with: the launcher and the library exchange engine data directly, so both must come from one engine version.

`scripts/src/lib.rs` exports a setup function with `aphyn_entry_point!`. It receives the same `AppInitCtx` as a static app, with the default modules already registered:

```rust
use aphyn::*;

#[derive(Component)]
struct Spinner {
    speed: f32,
}

fn spawn(mut entities: EntitiesHolderMut) {
    let e = entities.create_entity();
    entities.add_component(e, Spinner { speed: 1.0 }).ok().unwrap();
}

fn spin(time: Res<TimeResource>, mut q: WorldQuery<&mut Spinner>) {
    for spinner in q.iter_mut() {
        spinner.speed += time.delta_time;
    }
}

fn setup(mut ctx: AppInitCtx) {
    ctx.window.title = "My game".into();

    let mut behavior = ctx.behavior.as_mut();
    behavior.get_mut(Schedule::Start).insert_system(spawn);
    behavior.get_mut(Schedule::Update).insert_system(spin);
}

aphyn::aphyn_entry_point!(setup);
```

A game library holds exactly one `aphyn_entry_point!`: it defines the single symbol the launcher looks up.

`scripts/src/main.rs` starts the host and loads the library. Add your own startup code around the call if you need it:

```rust
fn main() {
    aphyn::launch_app_dynamically();
}
```

## Build it

Download the `aphyn` binary from [GitHub Releases](https://github.com/are-you-fruits-studio/aphyn/releases) and run:

```bash
aphyn build path/to/my_game
```

or run it from a checkout of the engine:

```bash
cargo run --release -p aphyn_editor -- build path/to/my_game
```

`aphyn build` runs `cargo build --release` on `scripts/`, then fills `builds/`:

| File | From |
|---|---|
| `my_game` (`my_game.exe` on Windows) | the `launcher` binary, renamed after the project folder |
| `aphynapp.dll` / `libaphynapp.so` | the game library |
| `assets/` | the project's `assets/`, if present |

Run the executable from `builds/`. At run time the game loads its assets from the `assets/` folder next to the executable.

### Requirements and errors

- `cargo` must be in your `PATH`.
- On Windows, keep the project path short (MSVC's 260-character path limit) and free of spaces.
- The build stops with an error if `scripts/src/main.rs` or the `[[bin]]` section named `launcher` is missing. Nothing is scaffolded for you.
- Older projects had a separate `launcher/` folder. It is no longer used; delete it.

## How the launcher finds your game

`launch_app_dynamically` creates the app, registers the default modules, then loads `aphynapp` (`aphynapp.dll`, `libaphynapp.so`, `libaphynapp.dylib`) from the folder of the running executable and calls the `aphyn_entry_point` symbol with the app's `AppInitCtx`. Your `setup` registers its components, systems and resources on that world, and the launcher runs the frame loop. The library stays loaded until the window closes, because the world holds its types and function pointers.

Because data crosses the library boundary as raw engine structures, a launcher and a library built from different engine versions fail at run time, not at compile time. See [Architecture](./concepts/architecture.md#two-ways-to-launch) for the static alternative.
