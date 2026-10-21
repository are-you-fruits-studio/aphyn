//! # fruits_window
//!
//! Window state as world data: [`WindowResource`] with its [`WindowState`] and
//! [`FullscreenState`]. The crate has no `winit` dependency, so lower-level crates (e.g.
//! `fruits_transform`) can read the window size without depending on `fruits_app`.
//!
//! # How to use
//!
//! `fruits_app` inserts [`WindowResource`] into the world when the window is created.
//! [`prev_state`](WindowResource::prev_state) is the state the real window had at the start of
//! the frame; read it for the current window size:
//!
//! ```ignore
//! use fruits_engine::*;
//!
//! fn log_window_size(window: Res<WindowResource>) {
//!     let [width, height] = window.prev_state().size;
//!     println!("{width}x{height}");
//! }
//! ```
//!
//! To change the window, write into [`next_state_mut`](WindowResource::next_state_mut); the
//! change is applied to the real window at the end of the frame:
//!
//! ```ignore
//! use fruits_engine::*;
//!
//! fn go_borderless(mut window: ResMut<WindowResource>) {
//!     window.next_state_mut().fullscreen = FullscreenState::Borderless;
//! }
//! ```
//!
//! # How to maintain
//!
//! The types here are plain `#[repr(C)]` data. Everything touching the real window
//! (reading a [`WindowState`] from it, applying the `prev`/`next` difference) lives in
//! `fruits_app`, which owns `winit`; see its "Double-buffered window state" section.

use fruits_ecs::Resource;
use fruits_ffi::FfiString;

#[repr(C)]
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum FullscreenState {
    Windowed,
    WindowedMaximized,
    Borderless,
    Exclusive,
}

#[repr(C)]
#[derive(Clone)]
pub struct WindowState {
    pub fullscreen: FullscreenState,
    pub size: [u32; 2],
    pub title: FfiString,
}

impl WindowState {
    pub fn reset(this: &mut Self, other: &WindowState) {
        this.fullscreen = other.fullscreen;
        this.size = other.size;
        this.title = other.title.clone();
    }
}

#[repr(C)]
#[derive(Resource)]
pub struct WindowResource {
    last_state: WindowState,
    next_state: WindowState,
}

impl WindowResource {
    pub fn new(state: WindowState) -> Self {
        Self {
            last_state: state.clone(),
            next_state: state,
        }
    }

    pub fn reset(this: &mut Self, new_state: &WindowState) {
        WindowState::reset(&mut this.last_state, new_state);
        WindowState::reset(&mut this.next_state, new_state);
    }

    pub fn prev_state(&self) -> &WindowState {
        &self.last_state
    }
    pub fn next_state(&self) -> &WindowState {
        &self.next_state
    }
    pub fn next_state_mut(&mut self) -> &mut WindowState {
        &mut self.next_state
    }
}
