use aphyn_ecs::{ProfileRecorder, Schedule, ThreadPoolAccess, WorldBehavior, WorldDataMut};
use aphyn_time::TimeResource;

pub fn run_frame_schedules(
    behavior: &mut WorldBehavior,
    mut data: WorldDataMut,
    real_delta_time: f32,
    thread_pool: ThreadPoolAccess,
    profiler: Option<&ProfileRecorder>,
) {
    let fixed_steps = time_mut(data.as_mut()).begin_frame(real_delta_time);

    for _ in 0..fixed_steps {
        time_mut(data.as_mut()).begin_fixed_step();
        behavior.execute_iteration(Schedule::FixedUpdate, data.as_mut(), thread_pool, profiler);
    }

    behavior.execute_iteration(Schedule::Update, data, thread_pool, profiler);
}

fn time_mut<'w>(data: WorldDataMut<'w>) -> &'w mut TimeResource {
    data.into_resources_mut()
        .into_get_mut::<TimeResource>()
        .expect("TimeResource must be inserted before running frame schedules")
}
