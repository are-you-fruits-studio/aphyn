---
title: Quick start
description: Install the toolchain, run an example, and write a first Aphyn app.
---

import AphynDependency from '@site/src/components/AphynDependency';

# Quick start

Aphyn is a 3D game engine in Rust: one Cargo workspace of small crates with an ECS at the core and rendering through [wgpu](https://wgpu.rs). This guide gets the engine building, runs an example, and walks through a first app.

:::warning Early development
The API changes between versions. Pin a [release tag](https://github.com/are-you-fruits-studio/aphyn/releases) when you build on Aphyn, and expect to follow breaking changes.
:::

## Requirements

- **Rust:** the latest stable toolchain (edition 2024). Aphyn uses recent language features, so update first:

  ```bash
  rustup update
  ```

- **Platform:** Windows (DirectX 12) or Linux (Vulkan), x86_64.
- **Linux packages** (Debian/Ubuntu):

  ```bash
  sudo apt-get update && sudo apt-get install -y libasound2-dev libudev-dev pkg-config
  ```

## Run an example

```bash
git clone https://github.com/are-you-fruits-studio/aphyn.git
cd aphyn
cargo run --release -p aphyn_example_cubes
```

The first release build takes a few minutes. Replace `cubes` with any of `audio_screen_saver`, `bloom`, `boids`, `collisions`, `ecs`, `gizmos`, `material`, `normals`, `physics`, `showcase`, `text` or `ui` (sources in [`crates/example/`](https://github.com/are-you-fruits-studio/aphyn/tree/dev/crates/example)). More standalone projects live in [aphyn.examples](https://github.com/are-you-fruits-studio/aphyn.examples).

## Your first app

Aphyn is not on crates.io yet. Depend on it through Git, pinned to a release tag:

<AphynDependency title="Cargo.toml" />

`src/main.rs`:

```rust
use aphyn::*;

#[derive(Resource)]
struct Frames {
    count: u64,
}

fn count_frames(mut frames: ResMut<Frames>) {
    frames.count += 1;
}

fn report(frames: Res<Frames>, input: Res<InputResource>) {
    if input.keyboard.is_just_pressed(KeyCode::Space) {
        println!("{} frames so far", frames.count);
    }
}

fn main() {
    let mut app = App::new();
    let AppInitCtx { mut data, mut behavior, window, render } = app.init_ctx_mut();

    add_defult_modules_to(data.as_mut(), behavior.as_mut(), render);
    window.title = "Hello, Aphyn".into();

    data.resources_mut().insert(Frames { count: 0 });

    let mut update = behavior.get_mut(Schedule::Update);
    update.insert_system(count_frames);
    update.insert_system(report);
    let _ = update.order_system(count_frames).before_system(report);

    app.run();
}
```

What it does:

- `App::new()` creates the world. `init_ctx_mut()` borrows its parts together: the world **data** (entities, resources, events), the **behavior** builder (systems per schedule), the initial **window** state, and the **render** feature registry.
- `add_defult_modules_to` registers every built-in subsystem: collision, physics, transforms, UI, rendering, audio and asset loading. The misspelling is the real name.
- `Frames` is a **resource**, one world-global value. Systems read it with `Res` and write it with `ResMut`.
- `count_frames` and `report` are **systems**: plain functions whose parameters declare the data they touch. The scheduler runs systems that do not conflict in parallel; `order_system(..).before_system(..)` pins an order where it matters.
- `app.run()` opens the window and blocks until it closes. Press <kbd>Space</kbd> to print the counter.

The window stays empty until the world has a camera and something to draw. The [`cubes` example](https://github.com/are-you-fruits-studio/aphyn/blob/dev/crates/example/cubes/src/main.rs) adds both.

## Read the API reference offline

The [API Reference](pathname:///api-reference/aphyn/index.html) on this site follows the `dev` branch. For the version you depend on, build it from that tag and keep it open next to your editor:

```bash
git checkout vX.Y.Z   # the tag in your Cargo.toml
cargo doc --workspace --no-deps --open
```

## Next steps

- [Build a game](./build-a-game.md): ship a game as a launcher, a game library and its assets.
- [Architecture](./concepts/architecture.md): how the crates fit together and what runs each frame.
- [ECS](./concepts/ecs.md): entities, components, resources, events, systems and schedules.
