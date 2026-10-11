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
//!     // Gamepads are keyed by id; an analog stick reads back as an axis value.
//!     if let Some(pad) = input.gamepads.get(&0) {
//!         let throttle = pad.read_axis(GamepadAxis::LeftStickY);
//!         let jumping = pad.is_just_pressed(GamepadButton::South);
//!         let _ = (throttle, jumping);
//!     }
//!
//!     let _ = (strafing, x, y);
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
//! [`WindowResource`] holds the window's desired state. Mutate its
//! [`next_state`](WindowResource::next_state_mut) in a system; the change is applied to the real
//! window at the end of the frame:
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
//! #### The event loop
//!
//! [`App::run`] hands the world builder to an internal `EventLoopHandler` and runs it under
//! `winit` with `ControlFlow::Poll`, so the loop keeps ticking even without OS events. The
//! handler is a three-state machine: it starts holding only the unbuilt `WorldBuilder`, moves
//! through a transient `Starting` state on the first `resumed` callback, and then stays in a
//! `Polling` state owning the built [`World`](fruits_ecs::World), the window, and the optional
//! gamepad host.
//!
//! On the first `resumed` the handler creates the window, builds the renderer's
//! `RenderApiResource`, inserts [`InputResource`] and [`WindowResource`] into the world,
//! finalizes the world, runs the [`Schedule::Start`](fruits_ecs::Schedule) pass once, and
//! initializes `gilrs` for gamepad support. A `gilrs` failure is logged and leaves gamepads
//! disabled rather than aborting.
//!
//! #### The frame
//!
//! Each `RedrawRequested` is one frame. Before the world runs, every gamepad `gilrs` currently
//! reports gets an entry in [`InputResource`], then pending `gilrs` events are drained into
//! it: connect/disconnect maintain the per-id gamepad map, and
//! button/axis events update the matching [`GamepadInputStorage`]. The
//! [`Schedule::Update`](fruits_ecs::Schedule) pass then runs, after which the handler clears the
//! frame's events, calls `clear_frame` on the input storages, reconciles the window state, and
//! requests the next redraw. Mouse, keyboard, and resize events arrive as their own `winit`
//! callbacks and write directly into the resources between frames.
//!
//! #### Frame-scoped input
//!
//! Each input storage keeps a persistent `pressed` set plus `frame_pressed` / `frame_released`
//! sets. `press`/`release` update both; `clear_frame` (called once per frame after `Update`)
//! empties only the two frame sets, which is what makes `is_just_pressed` / `is_just_released`
//! true for exactly one frame while `is_pressed` stays sticky. Repeated key events are dropped
//! in the window-event handler before anything else, so a held key neither re-triggers
//! `is_just_pressed` nor emits another [`TextInputEvent`].
//!
//! #### Double-buffered window state
//!
//! [`WindowResource`] holds a `prev` and a `next` [`WindowState`]. Systems write the desired
//! configuration into `next`; at frame end `WindowState::apply_difference` compares the two and
//! only touches the real window where they differ (currently the fullscreen mode), then copies
//! `next` into `prev`. This keeps redundant OS calls out of the steady state. The change is
//! applied only when the window reports a current monitor with at least one video mode
//! (exclusive fullscreen uses the first one), so without them any fullscreen switch —
//! including back to windowed — is a no-op, while `prev` is still updated.

mod app;
mod event_loop_handler;
mod input_resource;
mod window_resources;
mod window_events;

pub use app::*;
pub use input_resource::*;
pub use window_resources::*;
pub use window_events::*;
