---
title: ECS
description: Entities, components, resources, events, systems and schedules in aphyn_ecs.
---

# ECS

Aphyn keeps game state in an entity-component-system world (`aphyn_ecs`). Data lives in **entities** with **components**, world-global **resources**, and transient **events**. Logic lives in **systems**: plain functions whose parameters declare what they read and write, so the scheduler can run the ones that do not conflict in parallel.

## The world

A world has two sibling parts:

- **`WorldData`** holds the entities, resources and events.
- **`WorldBehaviorBuilder`** collects systems per schedule. It is built into a `WorldBehavior`, which runs one schedule pass at a time over the data.

An `App` owns both. You reach them through `app.init_ctx_mut()`, or through the `AppInitCtx` passed to `launch_app_statically` and to an `aphyn_entry_point!` setup.

## Components and entities

A component is a plain struct with `#[derive(Component)]`. An entity is an id; components attach to it:

```rust
use aphyn::*;

#[derive(Component)]
struct Position {
    x: f32,
    y: f32,
}

#[derive(Component)]
struct Velocity {
    x: f32,
    y: f32,
}

fn spawn(mut entities: EntitiesHolderMut) {
    let e = entities.create_entity();
    entities.add_component(e, Position { x: 0.0, y: 0.0 }).ok().unwrap();
    entities.add_component(e, Velocity { x: 1.0, y: 0.0 }).ok().unwrap();
}
```

Entities with the same set of component types share one contiguous store (an archetype), so queries scan only the stores that match. Adding or removing a component moves the entity to another archetype.

## Queries

A `WorldQuery` parameter iterates every entity that has the requested components. `&C` and `&mut C` are required; `Option<&C>` is fetched when present:

```rust
use aphyn::*;

#[derive(Component)]
struct Position {
    x: f32,
    y: f32,
}

#[derive(Component)]
struct Velocity {
    x: f32,
    y: f32,
}

fn integrate(mut q: WorldQuery<(&mut Position, &Velocity)>) {
    for (pos, vel) in q.iter_mut() {
        pos.x += vel.x;
        pos.y += vel.y;
    }
}
```

A second type argument filters without binding data: `WithFilter`, `WithoutFilter` and `OrFilter`.

```rust
use aphyn::*;

#[derive(Component)]
struct Enemy;

#[derive(Component)]
struct Health(u32);

#[derive(Component)]
struct Frozen;

fn total_active_enemy_health(q: WorldQuery<&Health, (WithFilter<Enemy>, WithoutFilter<Frozen>)>) {
    let total: u32 = q.iter().map(|h| h.0).sum();
    println!("{total}");
}
```

## Resources

A resource is one world-global value with `#[derive(Resource)]`. Read it with `Res`, write it with `ResMut`. Wrap either in `Option` if the resource may be missing; otherwise a missing resource panics the system.

```rust
use aphyn::*;

#[derive(Resource, Default)]
struct Score(u32);

fn add_point(mut score: ResMut<Score>) {
    score.0 += 1;
}

fn announce(score: Option<Res<Score>>) {
    if let Some(score) = score {
        println!("score is {}", score.0);
    }
}

fn register(mut data: WorldDataMut, mut behavior: WorldBehaviorBuilderMut) {
    data.resources_mut().insert(Score(0));

    let mut update = behavior.get_mut(Schedule::Update);
    update.insert_system(add_point);
    update.insert_system(announce);
}
```

The engine inserts its own resources, for example `InputResource` (keyboard, mouse, gamepads), `TimeResource` (frame and fixed-step time) and `WindowResource`.

## Events

An event is a transient message with `#[derive(Event)]`. Push with `EvtMut`, read the current frame's events with `Evt`. The app clears event buffers between frames.

```rust
use aphyn::*;

#[derive(Event)]
struct Damage {
    amount: u32,
}

fn deal_damage(mut writer: EvtMut<Damage>) {
    writer.push(Damage { amount: 10 });
}

fn apply_damage(reader: Evt<Damage>) {
    for damage in reader.iter() {
        println!("took {}", damage.amount);
    }
}
```

## Per-system state

A `Local<T>` parameter holds state private to one system. Any `'static + Default` type works, without a derive; it is created from `Default` the first time the system runs.

```rust
use aphyn::*;

#[derive(Default)]
struct Ticks(u64);

fn count_ticks(mut ticks: Local<Ticks>) {
    ticks.0 += 1;
}
```

## Structural changes

Creating or destroying entities and adding or removing components cannot run alongside systems that touch the same data. Such systems take a whole part of the world:

| Parameter | Claims |
|---|---|
| `EntitiesHolderMut` / `EntitiesHolderRef` | all entities and components |
| `ResourcesHolderMut` / `ResourcesHolderRef` | all resources |
| `EventsHolderMut` / `EventsHolderRef` | all events |
| `WorldDataMut` / `WorldDataRef` | all three |

A system with one of these never runs at the same time as another system using the same part.

## Schedules

Systems are registered per `Schedule`:

| Schedule | Runs |
|---|---|
| `Start` | once, before the first frame |
| `FixedUpdate` | zero or more times per frame, once per fixed time step; physics runs here |
| `Update` | once per frame, after the frame's `FixedUpdate` passes |

Read `TimeResource::delta_time` in `Update` and `fixed_delta_time` in `FixedUpdate`.

## Parallelism and ordering

When a schedule is built, every pair of systems whose data usage conflicts (one writes what the other reads or writes) gets an ordering edge; all other systems run in parallel on a thread pool. Two conflicting claims inside one system, such as two `ResMut` of the same type, panic when the system is registered.

Where order matters, pin it. Each `before_*` call orders the previous entry before the new one, so a call sequence builds a chain:

```rust
use aphyn::*;

fn read_input() {}
fn step_gameplay() {}
fn draw_debug() {}

fn register(mut behavior: WorldBehaviorBuilderMut) {
    let mut update = behavior.get_mut(Schedule::Update);
    update.insert_system(read_input);
    update.insert_system(step_gameplay);
    update.insert_system(draw_debug);

    // read_input → step_gameplay → draw_debug
    let _ = update
        .order_system(read_input)
        .before_system(step_gameplay)
        .before_system(draw_debug);

    // all three run before the engine's render group
    let _ = update.order_system(draw_debug).before_group(SYSTEM_GROUP_RENDER);
}
```

To put several systems after the same one, start a separate `order_system` for each edge. `group` bundles systems under a name so they can be ordered together, the way the engine's `SYSTEM_GROUP_*` groups are. Contradicting orders panic with a circular-dependency error when the schedule is built.

## Next

- [Architecture](./architecture.md): the default modules and their groups, and what runs each frame.
- The `aphyn_ecs` page of the [API Reference](pathname:///api-reference/aphyn_ecs/index.html): the full API and its internal design.
