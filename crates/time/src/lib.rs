//! # aphyn_time
//!
//! Frame and fixed-step time of the engine: [`TimeResource`] is the world resource every system reads to learn how much
//! time passed, and the render globals expose it to shaders.
//!
//! # How to use
//!
//! #### Time and fixed updates
//!
//! The app (`aphyn_app`) inserts [`TimeResource`] before the `Start` pass and advances it outside the ECS once per frame.
//! `delta_time`/`frame_time`/`frame_number` describe the frame (`Update`), while
//! `fixed_delta_time`/`fixed_frame_time`/`fixed_frame_number` describe the current
//! [`FixedUpdate`](aphyn_ecs::Schedule::FixedUpdate) step. Every frame the real elapsed time multiplied
//! by `time_scale` is accumulated, and one `FixedUpdate` pass runs per whole `fixed_time_step` in the
//! accumulator (zero, one or several per frame), always before that frame's `Update` pass. Systems may
//! write `time_scale` and `fixed_time_step`; they take effect from the next frame / step:
//!
//! ```
//! use aphyn_time::TimeResource;
//!
//! let mut time = TimeResource::default();
//! time.fixed_time_step = 0.02;
//!
//! assert_eq!(time.begin_frame(0.005), 0);
//! assert_eq!(time.begin_frame(0.1), 5);
//! for _ in 0..5 {
//!     time.begin_fixed_step();
//! }
//! assert_eq!(time.fixed_frame_number, 5);
//!
//! time.time_scale = 0.5;
//! assert_eq!(time.begin_frame(0.1), 2);
//! ```
//!
//! [`fixed_step_alpha`](TimeResource::fixed_step_alpha) is the time left in the accumulator after the
//! frame's fixed steps, as a fraction of `fixed_time_step` (0..=1). `Update` code uses it to blend
//! between the last two fixed steps:
//!
//! ```
//! use aphyn_time::TimeResource;
//!
//! let mut time = TimeResource::default();
//! time.fixed_time_step = 0.02;
//! assert_eq!(time.begin_frame(0.025), 1);
//! time.begin_fixed_step();
//! assert!((time.fixed_step_alpha() - 0.25).abs() < 1e-4);
//! ```
//!
//! `aphyn_app::run_frame_schedules` performs one whole frame (advance time, the due `FixedUpdate` passes, then
//! `Update`).
//!
//! #### Time in shaders
//!
//! `aphyn_render_features`' globals feature copies the frame time into the global uniform (`GlobalData` in WGSL):
//! `time` (`frame_time`), `delta_time`, `frame_number`, and `time_sin`/`time_cos`, the sine and cosine of
//! `(time / 8, time / 4, time / 2, time)`. All of it is scaled time, so `time_scale = 0` freezes shader animation too.
//! A world without a `TimeResource` renders with all zeros.
//!
//! # How to maintain
//!
//! - The type is `#[repr(C)]` because it lives in the world, which crosses the dynamic-library boundary; keep its fields
//!   ABI-stable.
//! - `accumulator` is private: only [`begin_frame`](TimeResource::begin_frame) adds to it and
//!   [`begin_fixed_step`](TimeResource::begin_fixed_step) drains it, one `fixed_time_step` per call. The caller must call
//!   `begin_fixed_step` exactly as many times as `begin_frame` returned, or the accumulator drifts.
//! - This crate depends only on `aphyn_ecs` so that render crates can read time without depending on `aphyn_app`, which
//!   depends on them. The frame driver (`run_frame_schedules`) stays in `aphyn_app`.
//! - The uniform layout that carries time lives in `aphyn_render` (`StandardUniformGlobal`); renaming fields here does
//!   not touch it.

use aphyn_ecs::Resource;

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

    pub fn fixed_step_alpha(&self) -> f32 {
        if self.fixed_time_step <= 0.0 {
            return 1.0;
        }
        (self.accumulator / self.fixed_time_step as f64).clamp(0.0, 1.0) as f32
    }
}
