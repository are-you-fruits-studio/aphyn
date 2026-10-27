//! # aphyn_app_launcher
//!
//! Entry points that start a `aphyn` application, either with the game
//! code compiled into the same binary or loaded from a separate dynamic library.
//!
//! # How to use
//!
//! These functions are re-exported from the `aphyn` facade crate, so an
//! application normally reaches them as `aphyn::launch_app_statically`
//! and `aphyn::launch_app_dynamically`. The examples below use the crate's
//! own paths so they type-check on their own.
//!
//! #### Launch a self-contained game
//!
//! Use this when the game's components, systems, and resources are compiled into
//! the same executable. Pass a closure that receives an [`AppInitCtx`]: populate
//! `ctx.world` and adjust the initial window through `ctx.window` (a `WindowState`
//! starting from its default; it is turned into the window's creation attributes).
//! Everything else (default engine modules, the window, the main loop) is set up for you.
//!
//! ```no_run
//! use aphyn_app::AppInitCtx;
//! use aphyn_app_launcher::launch_app_statically;
//!
//! fn main() {
//!     launch_app_statically(|ctx: AppInitCtx| {
//!         // Register your components, systems, and resources on `ctx.world`.
//!         ctx.window.title = "My game".into();
//!         ctx.window.size = [1280, 720];
//!     });
//! }
//! ```
//!
//! #### Launch a game from a dynamic library
//!
//! Use this when the game is built as a separate shared library (for example, a
//! thin launcher driven by the editor). The launcher binary only starts the host;
//! the game library supplies its setup through the `aphyn_entry_point!` macro.
//!
//! The launcher executable:
//!
//! ```ignore
//! fn main() {
//!     aphyn::launch_app_dynamically();
//! }
//! ```
//!
//! The game library exposes its setup function to the launcher:
//!
//! ```ignore
//! use aphyn::*;
//!
//! fn setup(ctx: AppInitCtx) {
//!     // Register your components, systems, and resources on `ctx.world`.
//!     ctx.window.fullscreen = FullscreenState::Borderless;
//! }
//!
//! aphyn::aphyn_entry_point!(setup);
//! ```
//!
//! # How to maintain
//!
//! Both entry points follow the same three steps: build an [`App`], register the
//! engine's default modules with [`aphyn_modules::add_defult_modules_to`], let the
//! game populate the world, then hand control to [`App::run`]. They differ only in
//! *how* the game populates the world.
//!
//! [`launch_app_statically`] is the simple path: the caller's closure is invoked
//! directly with the [`AppInitCtx`] from [`App::init_ctx_mut`], which borrows the
//! in-process [`App`]'s world builder and initial window state.
//!
//! [`launch_app_dynamically`] crosses an FFI boundary. It looks for a shared library
//! named `lib_app` plus the platform's library suffix (`lib_app.dll`, `lib_app.so`, …)
//! next to the current executable and hands it, with the app's [`AppInitCtx`], to
//! [`init_app_dynamically`]. That function loads the library via `libloading` and passes it
//! to [`init_app_from_library`], which resolves the `aphyn_entry_point` C symbol (an
//! `unsafe extern "C-unwind" fn(AppInitCtxFfi)` that the game library exports through the
//! `aphyn_entry_point!` macro) and calls it with `AppInitCtx::into_ffi`: the world builder,
//! type registry and window state packed as raw pointers into an [`AppInitCtxFfi`]. The
//! library reconstructs a safe [`AppInitCtx`] on the other side and runs the game's setup.
//! [`init_app_from_library`] can be called again on an already-loaded library with another
//! ctx to set up another world (the editor rebuilds its simulated world this way when
//! entering and leaving play mode, passing that world's simulated window state); every such
//! world must be dropped before the library. [`init_app_dynamically`] returns the loaded [`Library`] (or the
//! `libloading` error), and the library is closed only after [`App::run`] returns, because
//! the world holds types and function pointers that live inside it for the whole run.
//!
//! Caveats for maintainers: [`launch_app_dynamically`] `unwrap`s every fallible step
//! (locating the executable, loading `lib_app`, resolving the symbol, closing the
//! library), so a missing or mismatched library aborts the process rather than reporting
//! an error. The default-module
//! helper name `add_defult_modules_to` carries an upstream spelling and must match
//! `aphyn_modules`.

use std::path::Path;

use aphyn_app::{App, AppInitCtx, AppInitCtxFfi};
use libloading::Library;

pub fn launch_app_statically(f: impl FnOnce(AppInitCtx)) {
    let mut app = App::new();

    aphyn_modules::add_defult_modules_to(app.ecs_mut().as_mut());

    f(app.init_ctx_mut());

    app.run();
}

pub fn launch_app_dynamically() {
    let mut app = App::new();

    aphyn_modules::add_defult_modules_to(app.ecs_mut().as_mut());

    let lib_path = std::env::current_exe().unwrap().with_file_name(format!("lib_app{}", std::env::consts::DLL_SUFFIX));
    let lib = unsafe { init_app_dynamically(app.init_ctx_mut(), lib_path).unwrap() };

    app.run();

    lib.close().unwrap();
}

pub unsafe fn init_app_dynamically(ctx: AppInitCtx, lib_path: impl AsRef<Path>) -> Result<Library, libloading::Error> {
    let lib = unsafe { libloading::Library::new(lib_path.as_ref())? };

    unsafe { init_app_from_library(ctx, &lib)? };

    Ok(lib)
}

pub unsafe fn init_app_from_library(ctx: AppInitCtx, lib: &Library) -> Result<(), libloading::Error> {
    let init_app_symbol = unsafe { lib.get::<unsafe extern "C-unwind" fn(AppInitCtxFfi)>(b"aphyn_entry_point")? };

    unsafe { init_app_symbol(ctx.into_ffi()) };

    Ok(())
}