mod capture;
mod flock;
mod mesh;
mod scene;

use std::path::PathBuf;

use aphyn::*;

pub const WINDOW_SIZE: [u32; 2] = [1280, 720];
pub const SIM_HZ: u32 = 60;
pub const SIM_DT: f32 = 1.0 / SIM_HZ as f32;
pub const LOOP_SECONDS: u32 = 7;
pub const LOOP_STEPS: u32 = SIM_HZ * LOOP_SECONDS;
pub const CAPTURE_FPS: u32 = 30;
pub const WARMUP_LOOPS: u32 = 4;

#[derive(Resource, Default)]
pub struct LoopClock {
    pub step: u64,
}

impl LoopClock {
    pub fn phase(&self) -> f32 {
        (self.step % LOOP_STEPS as u64) as f32 / LOOP_STEPS as f32
    }
}

fn main() {
    let capture_dir = parse_capture_dir();

    let mut app = App::new();

    let ctx = app.init_ctx_mut();
    ctx.window.size = WINDOW_SIZE;
    ctx.window.is_resizable = false;
    ctx.window.title = "aphyn showcase".into();

    add_defult_modules_to(app.ecs_mut().as_mut());

    let ecs = app.ecs_mut();
    ecs.data_mut().resources_mut().insert(LoopClock::default());

    let mut behavior = ecs.behavior_mut();

    let mut start = behavior.get_mut(Schedule::Start);
    start.insert_system(scene::setup_post_processing);
    start.insert_system(scene::setup_scene);
    start.order_group(SYSTEM_GROUP_RENDER).before_system(scene::setup_scene);
    start.order_group(SYSTEM_GROUP_UI).before_system(scene::setup_scene);

    let mut update = behavior.get_mut(Schedule::Update);
    update.insert_system(advance_clock);
    update.insert_system(flock::seek_targets);
    update.insert_system(flock::accumulate_separation);
    update.insert_system(flock::apply_steering);
    update.insert_system(flock::integrate_boids);
    update.insert_system(flock::move_flock_lights);
    update.insert_system(scene::orbit_camera);
    update.insert_system(scene::update_entity_count);
    update.insert_system(scene::draw_gizmos);

    update.order_system(advance_clock).before_system(flock::seek_targets);
    update.order_system(advance_clock).before_system(flock::accumulate_separation);
    update.order_system(advance_clock).before_system(flock::move_flock_lights);
    update.order_system(advance_clock).before_system(scene::orbit_camera);
    update.order_system(flock::seek_targets).before_system(flock::apply_steering);
    update
        .order_system(flock::accumulate_separation)
        .before_system(flock::apply_steering);
    update
        .order_system(flock::apply_steering)
        .before_system(flock::integrate_boids);
    update.order_system(scene::draw_gizmos).before_group(SYSTEM_GROUP_RENDER);

    if let Some(dir) = capture_dir {
        update.insert_system(capture::measure_loop_seam);
        update
            .order_system(flock::integrate_boids)
            .before_system(capture::measure_loop_seam);

        capture::register(app.ecs_mut().as_mut(), dir);
    }

    app.run();
}

fn advance_clock(mut clock: ResMut<LoopClock>) {
    clock.step += 1;
}

fn parse_capture_dir() -> Option<PathBuf> {
    let mut args = std::env::args().skip(1);

    while let Some(arg) = args.next() {
        if arg == "--capture" {
            return Some(PathBuf::from(args.next().expect("--capture needs a directory")));
        }
    }

    None
}
