use std::{collections::VecDeque, time::Instant};

use fruits_ecs::{ResMut, Resource, Schedule, WorldBuilderMut};

// todo: ffi?

pub fn add_module_to(mut world: WorldBuilderMut) {
    world.data_mut().resources_mut().insert(FpsResource::default());

    world.behavior_mut().get_mut(Schedule::Update).insert_system(count_fps_system);
}

#[derive(Resource, Default)]
struct FpsResource {
    last_frame_times_s: VecDeque<f64>,
    last_frame_time: Option<Instant>,
    last_print_time: Option<Instant>,
    last_fps: usize,
    last_frame_time_ms: f64,
}

impl FpsResource {
    // todo
    // pub fn last_frame_times_s(&self) -> &VecDeque<f64> {
    //     &self.last_frame_times_s
    // }
    // pub fn last_fps(&self) -> usize {
    //     self.last_fps
    // }
    // pub fn last_frame_time_ms(&self) -> f64 {
    //     self.last_frame_time_ms
    // }
}

fn count_fps_system(mut res: ResMut<FpsResource>) {
    const MAX_FRAMES: usize = 100;
    const PRINT_PERIOD_S: f64 = 1.0;

    if res.last_frame_times_s.len() >= MAX_FRAMES {
        res.last_frame_times_s.pop_front();
    }

    let last_frame_time = res.last_frame_time;

    res.last_frame_time = Some(Instant::now());

    let Some(last_frame_time) = last_frame_time else {
        return;
    };

    let duration = last_frame_time.elapsed();
    res.last_frame_times_s.push_back(duration.as_secs_f64());

    if res.last_frame_times_s.is_empty() {
        return;
    }

    let Some(last_print_time) = res.last_print_time else {
        res.last_print_time = Some(Instant::now());
        return;
    };

    if last_print_time.elapsed().as_secs_f64() < PRINT_PERIOD_S {
        return;
    }

    let avg_frame_time_s = res.last_frame_times_s.iter().sum::<f64>() / res.last_frame_times_s.len() as f64;
    let avg_frame_time_ms = avg_frame_time_s * 1000.0;
    let fps = (1.0 / avg_frame_time_s) as usize;

    res.last_fps = fps;
    res.last_frame_time_ms = avg_frame_time_ms;

    println!("fps: {: >5} | frame_time: {: >10.3} ms", fps, avg_frame_time_ms);
    res.last_print_time = Some(Instant::now());
}
