//! # fruits_app
//!
//! The runtime entry point of the Fruits engine: it opens a window, drives the per-frame
//! loop, and feeds keyboard, mouse, and gamepad input into the ECS world.
//!
//! # How to use
//!
//! #### Creating and running an app
//!
//! [`App`] wraps the engine's world builder. Register systems and seed entities on
//! [`ecs_mut`](App::ecs_mut), then call [`run`](App::run) to open the window and start the
//! frame loop. `run` blocks until the window closes.
//!
//! ```ignore
//! use fruits_engine::*;
//!
//! fn main() {
//!     let mut app = App::new();
//!
//!     // Pull in the engine's default subsystems (collision, transform, rendering, ...).
//!     add_defult_modules_to(app.ecs_mut().as_mut());
//!
//!     app.ecs_mut()
//!         .behavior_mut()
//!         .get_mut(Schedule::Update)
//!         .insert_system(move_player);
//!
//!     app.run();
//! }
//!
//! fn move_player(input: Res<InputResource>) {
//!     if input.keyboard.is_pressed(KeyCode::KeyW) {
//!         // advance the player
//!     }
//! }
//! ```
//!
//! #### Configuring the window before it opens
//!
//! [`App`] holds the initial [`WindowState`](fruits_window::WindowState) (its `Default`: windowed
//! 800x600, titled "fruits_engine app"). [`init_ctx_mut`](App::init_ctx_mut) returns an
//! [`AppInitCtx`] borrowing both the world builder and that state; whatever the state holds when
//! [`run`](App::run) creates the window is used as the window's creation attributes. The same ctx
//! is what `launch_app_statically` and a game library's `fruits_entry_point!` setup receive.
//!
//! ```ignore
//! use fruits_engine::*;
//!
//! let mut app = App::new();
//! let ctx = app.init_ctx_mut();
//! ctx.window.title = "My game".into();
//! ctx.window.fullscreen = FullscreenState::WindowedMaximized;
//! app.run();
//! ```
//!
//! #### Reading keyboard, mouse, and gamepad input
//!
//! [`InputResource`] is inserted into the world before the [`Start`](fruits_ecs::Schedule::Start) pass and
//! refreshed every frame. Read it in a system with `Res<InputResource>`. Each device
//! distinguishes a held state (`is_pressed`) from edge states that are true for a single frame
//! (`is_just_pressed`, `is_just_released`):
//!
//! ```ignore
//! use fruits_engine::*;
//!
//! fn fire(input: Res<InputResource>) {
//!     // Held this frame.
//!     let strafing = input.keyboard.is_pressed(KeyCode::KeyA);
//!
//!     // True only on the frame the button went down.
//!     if input.mouse.is_just_pressed(MouseButton::Left) {
//!         // shoot once
//!     }
//!
//!     // Cursor position in physical pixels.
//!     let [x, y] = input.mouse.position;
//!
//!     // Wheel scroll accumulated since the last frame; positive `y` scrolls up / away from the user.
//!     // Notched wheels report lines, touchpads report physical pixels, so read both.
//!     let [_, wheel_lines] = input.mouse.scroll_delta_lines;
//!     let [_, wheel_pixels] = input.mouse.scroll_delta_pixels;
//!
//!     // Gamepads are keyed by id; an analog stick reads back as an axis value.
//!     if let Some(pad) = input.gamepads.get(&0) {
//!         let throttle = pad.read_axis(GamepadAxis::LeftStickY);
//!         let jumping = pad.is_just_pressed(GamepadButton::South);
//!         let _ = (throttle, jumping);
//!     }
//!
//!     let _ = (strafing, x, y, wheel_lines, wheel_pixels);
//! }
//! ```
//!
//! #### Reacting to typed text
//!
//! The text a key press produces (as resolved by the platform's keyboard layout) is emitted as
//! a [`TextInputEvent`]. Read it with `Evt<TextInputEvent>` — useful for text fields, where raw
//! key codes are not enough. Key repeats and IME composition are not delivered:
//!
//! ```ignore
//! use fruits_engine::*;
//!
//! fn type_into_field(text_input: Evt<TextInputEvent>) {
//!     for TextInputEvent(text) in text_input.iter() {
//!         // `text` is the typed string; "\u{8}" is backspace.
//!         let _ = text;
//!     }
//! }
//! ```
//!
//! #### Switching fullscreen mode
//!
//! [`WindowResource`](fruits_window::WindowResource) (from `fruits_window`) holds the window's
//! state and pending change requests. Request a change in a system with
//! [`set_fullscreen`](fruits_window::WindowResource::set_fullscreen) (or `set_size` / `set_title`);
//! it is applied to the real window at the end of the frame:
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
//! #### The event loop
//!
//! [`App::run`] hands the world builder and the initial window state to an internal
//! `EventLoopHandler` and runs it under `winit` with `ControlFlow::Poll`, so the loop keeps
//! ticking even without OS events. The handler is a three-state machine: it starts holding only
//! the unbuilt `WorldBuilder` and the `WindowState`, moves through a transient `Starting` state
//! on the first `resumed` callback, and then stays in a `Polling` state owning the built
//! [`World`](fruits_ecs::World), the window, and the optional gamepad host.
//!
//! On the first `resumed` the handler creates the window from the window state
//! (`window_attributes_from_state`: title, inner size, resizable, icon, maximized and fullscreen;
//! exclusive fullscreen uses the primary monitor's first video mode, else borderless), applies
//! cursor grab and visibility to the created window (no creation attributes exist for them),
//! re-reads fullscreen mode and size from it, builds the renderer's
//! `RenderApiResource`, inserts [`InputResource`] and [`WindowResource`](fruits_window::WindowResource) into the world,
//! finalizes the world, runs the [`Schedule::Start`](fruits_ecs::Schedule) pass once, applies the
//! window state it requested, and initializes `gilrs` for gamepad support. A `gilrs` failure is logged and leaves gamepads
//! disabled rather than aborting.
//!
//! #### The frame
//!
//! Each `RedrawRequested` is one frame. Before the world runs, the window state is read back from
//! the real window, and every gamepad `gilrs` currently
//! reports gets an entry in [`InputResource`], then pending `gilrs` events are drained into
//! it: connect/disconnect maintain the per-id gamepad map, and
//! button/axis events update the matching [`GamepadInputStorage`]. The
//! [`Schedule::Update`](fruits_ecs::Schedule) pass then runs, followed by the render core system,
//! after which the handler clears the frame's events, calls `clear_frame` on the input storages,
//! reconciles the window state, and requests the next redraw. Mouse (buttons, cursor, wheel), keyboard, and resize events
//! arrive as their own `winit` callbacks and write directly into the resources between frames.
//!
//! #### Rendering
//!
//! The handler owns the frame's color target outside the world: an `Rgba16Float` texture
//! (`RENDER_TARGET_FORMAT` from `fruits_render_core`) the size of the surface, rebuilt whenever the
//! `RenderApiResource` size changes, plus the pipeline that copies it onto the surface
//! (`assets/copy_texture.wgsl`, nearest sampling, `REPLACE` blend). Each frame the render
//! core system clears the target, takes the first entity with a `GlobalTransform` and a
//! `CameraComponent`, inserts a `CurrentRenderingResource` describing that camera and the target,
//! runs the [`Schedule::RenderCamera`](fruits_ecs::Schedule) pass, and removes the resource again.
//! Further cameras are ignored for now, and with no camera the pass does not run. Finally the
//! target is copied onto the surface texture and presented; if the surface texture cannot be
//! acquired the presentation is skipped for that frame.
//!
//! #### Frame-scoped input
//!
//! Each input storage keeps a persistent `pressed` set plus `frame_pressed` / `frame_released`
//! sets. `press`/`release` update both; `clear_frame` (called once per frame after `Update`)
//! empties only the two frame sets, which is what makes `is_just_pressed` / `is_just_released`
//! true for exactly one frame while `is_pressed` stays sticky. The mouse storage's `clear_frame`
//! also zeroes `scroll_delta_lines` / `scroll_delta_pixels`, which the wheel handler accumulates
//! into (`+=`) so several wheel events between two frames add up. Repeated key events are dropped
//! in the window-event handler before anything else, so a held key neither re-triggers
//! `is_just_pressed` nor emits another [`TextInputEvent`].
//!
//! #### Window state and change requests
//!
//! [`WindowResource`](fruits_window::WindowResource) holds a [`WindowState`](fruits_window::WindowState)
//! and [`WindowChangeRequests`](fruits_window::WindowChangeRequests) (one optional value per field).
//! Systems only add requests; after the `Update` pass (and once after the `Start` pass)
//! `window_state_apply_changes` applies each request that differs from the current state to the
//! real window, which keeps redundant OS calls out of the steady state. The requests stay in the
//! resource until the next frame start, where they are taken and moved into the state in place
//! (through `WindowResource::state_mut`, so icon pixels are never cloned per frame), and then
//! fullscreen mode and size are re-read from the real window (`window_state_refresh_from_window`)
//! so the state reflects user resizes and requests the OS did not honor. The title, the
//! resizable flag, the icon, and cursor grab and visibility are never read back from the window
//! (`winit` cannot read the title on x11, Wayland, iOS, Android and Web, resizable on x11, iOS,
//! Android and Web, and the icon and cursor state nowhere), so they start as the initial window
//! state the window was created from and are afterwards kept from the last request. An icon whose
//! pixel buffer does not match its size is logged and falls back to the platform default. A grab
//! mode the platform rejects (`Locked` on Windows and x11, `Confined` on macOS, ...) is reset to
//! `CursorGrabState::None` at creation and removed from the requests in
//! `window_state_apply_changes` later, so the state keeps the previous mode. Exclusive fullscreen
//! uses the current monitor's first video mode and falls back to borderless when there is none.
//!
//! #### Init ctx
//!
//! [`AppInitCtx`] lives here rather than in `fruits_ecs` because it carries a
//! `fruits_window::WindowState`, and `fruits_window` depends on `fruits_ecs`. [`AppInitCtxFfi`] is
//! its `#[repr(C)]` form for the dynamic-library boundary: raw pointers to the world builder, the
//! type registry, and the window state. `AppInitCtx::into_ffi` packs them on the host side and
//! `AppInitCtxFfi::init_app` unpacks them inside the game library (called by the
//! `fruits_entry_point!` expansion). The pointers are only valid while the ctx's borrows are,
//! and the layout must match on both sides of the boundary.

mod app;
mod event_loop_handler;
mod init_ctx;
mod render_target;
mod input_resource;
mod window_resources;
mod window_events;

pub use app::*;
pub use init_ctx::*;
pub use input_resource::*;
pub use window_events::*;
