use std::sync::{LazyLock, Mutex};

use fruits_engine::*;

fn main() {
    let mut app = App::new();

    let world = app.ecs_mut();

    let mut world_behavior = world.behavior_mut();

    world_behavior
        .get_mut(Schedule::Update)
        .insert_system(create_and_destroy_entity);
    world_behavior
        .get_mut(Schedule::Update)
        .insert_system(test_resource_existence);
    world_behavior.get_mut(Schedule::Update).insert_system(test_optionals);

    //app.ecs_mut().data_mut().resources_mut().insert(SomeResource).ok().unwrap();

    let mut world_data = world.data_mut();

    let mut ec = world_data.entities_mut();

    for i in 0..10 {
        let e = ec.create_entity();

        ec.add_component(e, SomeComponent1).ok().unwrap();

        if i % 2 == 0 {
            ec.add_component(e, SomeComponent2).ok().unwrap();
        }
    }

    app.run();
}

fn create_and_destroy_entity(mut w: WorldDataMut) {
    let mut ec = w.entities_mut();

    let e = ec.create_entity();

    ec.add_component(e, MaliciousComponent(Vec::with_capacity(8))).ok().unwrap();
    ec.add_component(e, LoudComponent::new()).ok().unwrap();

    ec.remove_component::<MaliciousComponent>(e);

    ec.destroy_entity(e);
}

fn test_resource_existence(res: Option<Res<SomeResource>>) {
    dbg!(res.is_some());
}

fn test_optionals(mut q: WorldQuery<(&SomeComponent1, Option<&mut SomeComponent2>)>) {
    dbg!(q.len());
    for (_c1, c2) in q.iter_mut() {
        dbg!(c2.is_some());
    }
}

#[derive(Resource)]
struct SomeResource;

#[derive(Component)]
struct SomeComponent1;

#[derive(Component)]
struct SomeComponent2;

#[derive(Component)]
struct MaliciousComponent(
    #[allow(dead_code)]
    Vec<u8>
);

#[derive(Component)]
struct LoudComponent {
    id: usize,
}

static LOUD_COUNTER: LazyLock<Mutex<usize>> = LazyLock::new(|| Mutex::new(0));

impl LoudComponent {
    pub fn new() -> Self {
        let mut counter = LOUD_COUNTER.lock().unwrap();
        let id = *counter;
        *counter += 1;

        println!("louddddd created {}", id);
        Self { id }
    }
}

impl Drop for LoudComponent {
    fn drop(&mut self) {
        println!("louddddd destroyed {}", self.id);
    }
}
