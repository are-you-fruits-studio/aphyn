use aphyn_ecs::{Resource, Schedule, WorldBehavior, WorldDataMut};

#[repr(C)]
#[derive(Resource, Clone, Debug)]
pub struct TimeResource {
    pub frame_number: u64,
    pub frame_time: f64,
    pub fixed_frame_number: u64,
    pub fixed_frame_time: f64,
    pub delta_time: f32,
    pub fixed_delta_time: f32,
    pub time_scale: f32,
    pub fixed_time_step: f32,
    accumulator: f64,
}

impl Default for TimeResource {
    fn default() -> Self {
        Self {
            frame_number: 0,
            frame_time: 0.0,
            fixed_frame_number: 0,
            fixed_frame_time: 0.0,
            delta_time: 0.0,
            fixed_delta_time: Self::DEFAULT_FIXED_TIME_STEP,
            time_scale: 1.0,
            fixed_time_step: Self::DEFAULT_FIXED_TIME_STEP,
            accumulator: 0.0,
        }
    }
}

impl TimeResource {
    pub const DEFAULT_FIXED_TIME_STEP: f32 = 1.0 / 50.0;

    pub fn begin_frame(&mut self, real_delta_time: f32) -> u32 {
        self.frame_number += 1;
        self.delta_time = real_delta_time.max(0.0) * self.time_scale.max(0.0);
        self.frame_time += self.delta_time as f64;
        self.accumulator += self.delta_time as f64;

        // todo: clamp catch-up steps so long frames can't freeze the app
        let step = self.fixed_time_step as f64;
        if step <= 0.0 {
            return 0;
        }

        (self.accumulator / step).floor() as u32
    }

    pub fn begin_fixed_step(&mut self) {
        self.fixed_delta_time = self.fixed_time_step;
        self.fixed_frame_number += 1;
        self.fixed_frame_time += self.fixed_time_step as f64;
        self.accumulator -= self.fixed_time_step as f64;
    }
}

pub fn run_frame_schedules(behavior: &mut WorldBehavior, mut data: WorldDataMut, real_delta_time: f32) {
    let fixed_steps = time_mut(data.as_mut()).begin_frame(real_delta_time);

    for _ in 0..fixed_steps {
        time_mut(data.as_mut()).begin_fixed_step();
        behavior.execute_iteration(Schedule::FixedUpdate, data.as_mut());
    }

    behavior.execute_iteration(Schedule::Update, data);
}

fn time_mut<'w>(data: WorldDataMut<'w>) -> &'w mut TimeResource {
    data.into_resources_mut()
        .into_get_mut::<TimeResource>()
        .expect("TimeResource must be inserted before running frame schedules")
}
