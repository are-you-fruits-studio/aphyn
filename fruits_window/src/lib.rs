//! # fruits_window
//!
//! Window state as world data: [`WindowResource`] with its [`WindowState`] and
//! [`FullscreenState`]. The crate has no `winit` dependency, so lower-level crates (e.g.
//! `fruits_transform`) can read the window size without depending on `fruits_app`.
//!
//! # How to use
//!
//! The window is created from an initial [`WindowState`] the app owns (starting from
//! [`WindowState::default`]: windowed 800x600, titled "fruits_engine app", resizable, no icon,
//! free visible cursor), which the game sets through `fruits_app`'s `AppInitCtx` before the
//! window opens. `fruits_app` then inserts [`WindowResource`] into the world.
//! [`state`](WindowResource::state) is the window state at the start of the frame; read it for
//! the current window size:
//!
//! ```ignore
//! use fruits_engine::*;
//!
//! fn log_window_size(window: Res<WindowResource>) {
//!     let [width, height] = window.state().size;
//!     println!("{width}x{height}");
//! }
//! ```
//!
//! To change the window, request it with [`set_fullscreen`](WindowResource::set_fullscreen),
//! [`set_size`](WindowResource::set_size), [`set_title`](WindowResource::set_title),
//! [`set_resizable`](WindowResource::set_resizable), [`set_icon`](WindowResource::set_icon)
//! (a [`WindowIcon`] of RGBA8 pixels, `None` for the platform default),
//! [`set_cursor_grab`](WindowResource::set_cursor_grab) (a [`CursorGrabState`]) or
//! [`set_cursor_visible`](WindowResource::set_cursor_visible); the request is applied to the real window at the end of the frame and shows up in
//! [`state`](WindowResource::state) from the next frame on. A cursor grab mode the platform does
//! not support is dropped, so the state keeps the previous mode:
//!
//! ```ignore
//! use fruits_engine::*;
//!
//! fn go_borderless(mut window: ResMut<WindowResource>) {
//!     window.set_fullscreen(FullscreenState::Borderless);
//! }
//! ```
//!
//! # How to maintain
//!
//! The types here are plain `#[repr(C)]` data. Everything touching the real window
//! (reading a [`WindowState`] from it, applying [`WindowChangeRequests`]) lives in
//! `fruits_app`, which owns `winit`; see its "Window state and change requests" section.
//! [`state_mut`](WindowResource::state_mut) is an associated function rather than a method so
//! game systems reach for the `set_*` requests instead; it is meant for the frame-start sync in
//! `fruits_app` and for hosts that simulate a window (the editor's scene view). The state is
//! updated in place there, never cloned, because it can hold icon pixel data.

use fruits_ecs::Resource;
use fruits_ffi::{FfiOption, FfiString, FfiVec};

#[repr(C)]
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum FullscreenState {
    Windowed,
    WindowedMaximized,
    Borderless,
    Exclusive,
}

#[repr(C)]
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum CursorGrabState {
    None,
    Confined,
    Locked,
}

#[repr(C)]
#[derive(Clone)]
pub struct WindowState {
    pub fullscreen: FullscreenState,
    pub size: [u32; 2],
    pub title: FfiString,
    pub is_resizable: bool,
    pub icon: FfiOption<WindowIcon>,
    pub cursor_grab: CursorGrabState,
    pub is_cursor_visible: bool,
}

impl Default for WindowState {
    fn default() -> Self {
        Self {
            fullscreen: FullscreenState::Windowed,
            size: [800, 600],
            title: "fruits_engine app".into(),
            is_resizable: true,
            icon: FfiOption::None,
            cursor_grab: CursorGrabState::None,
            is_cursor_visible: true,
        }
    }
}

impl WindowState {
    pub fn apply_changes(this: &mut Self, changes: WindowChangeRequests) {
        if let Some(fullscreen) = changes.fullscreen.into_option() {
            this.fullscreen = fullscreen;
        }
        if let Some(size) = changes.size.into_option() {
            this.size = size;
        }
        if let Some(title) = changes.title.into_option() {
            this.title = title;
        }
        if let Some(resizable) = changes.is_resizable.into_option() {
            this.is_resizable = resizable;
        }
        if let Some(icon) = changes.icon.into_option() {
            this.icon = icon;
        }
        if let Some(cursor_grab) = changes.cursor_grab.into_option() {
            this.cursor_grab = cursor_grab;
        }
        if let Some(cursor_visible) = changes.is_cursor_visible.into_option() {
            this.is_cursor_visible = cursor_visible;
        }
    }
}

#[repr(C)]
#[derive(Clone, PartialEq)]
pub struct WindowIcon {
    pub rgba: FfiVec<u8>,
    pub size: [u32; 2],
}

#[repr(C)]
#[derive(Default)]
pub struct WindowChangeRequests {
    pub fullscreen: FfiOption<FullscreenState>,
    pub size: FfiOption<[u32; 2]>,
    pub title: FfiOption<FfiString>,
    pub is_resizable: FfiOption<bool>,
    pub icon: FfiOption<FfiOption<WindowIcon>>,
    pub cursor_grab: FfiOption<CursorGrabState>,
    pub is_cursor_visible: FfiOption<bool>,
}

#[repr(C)]
#[derive(Resource)]
pub struct WindowResource {
    state: WindowState,
    change_requests: WindowChangeRequests,
}

impl WindowResource {
    pub fn new(state: WindowState) -> Self {
        Self {
            state,
            change_requests: WindowChangeRequests::default(),
        }
    }

    pub fn state(&self) -> &WindowState {
        &self.state
    }
    pub fn state_mut(this: &mut Self) -> &mut WindowState {
        &mut this.state
    }
    pub fn take_changes(this: &mut Self) -> WindowChangeRequests {
        std::mem::take(&mut this.change_requests)
    }
    pub fn changes(this: &Self) -> &WindowChangeRequests {
        &this.change_requests
    }
    pub fn state_and_changes_mut(this: &mut Self) -> (&WindowState, &mut WindowChangeRequests) {
        (&this.state, &mut this.change_requests)
    }

    pub fn set_fullscreen(&mut self, fullscreen: FullscreenState) {
        self.change_requests.fullscreen = fullscreen.into();
    }
    pub fn set_size(&mut self, size: [u32; 2]) {
        self.change_requests.size = size.into();
    }
    pub fn set_title(&mut self, title: FfiString) {
        self.change_requests.title = title.into();
    }
    pub fn set_resizable(&mut self, resizable: bool) {
        self.change_requests.is_resizable = resizable.into();
    }
    pub fn set_icon(&mut self, icon: Option<WindowIcon>) {
        self.change_requests.icon = FfiOption::Some(icon.into());
    }
    pub fn set_cursor_grab(&mut self, cursor_grab: CursorGrabState) {
        self.change_requests.cursor_grab = cursor_grab.into();
    }
    pub fn set_cursor_visible(&mut self, cursor_visible: bool) {
        self.change_requests.is_cursor_visible = cursor_visible.into();
    }
}
