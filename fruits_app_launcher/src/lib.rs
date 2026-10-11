//! # fruits_app_launcher
//!
//! Entry points that start a `fruits_engine` application, either with the game
//! code compiled into the same binary or loaded from a separate dynamic library.
//!
//! # How to use
//!
//! These functions are re-exported from the `fruits_engine` facade crate, so an
//! application normally reaches them as `fruits_engine::launch_app_statically`
//! and `fruits_engine::launch_app_dynamically`. The examples below use the crate's
//! own paths so they type-check on their own.
//!
//! #### Launch a self-contained game
//!
//! Use this when the game's components, systems, and resources are compiled into
//! the same executable. Pass a closure that populates the world; everything else
//! (default engine modules, the window, the main loop) is set up for you.
//!
//! ```no_run
//! use fruits_app_launcher::launch_app_statically;
//! use fruits_ecs::WorldBuilderMut;
//!
//! fn main() {
//!     launch_app_statically(|world: WorldBuilderMut| {
//!         // Register your components, systems, and resources on `world`.
//!         let _ = world;
//!     });
//! }
//! ```
//!
//! #### Launch a game from a dynamic library
//!
//! Use this when the game is built as a separate shared library (for example, a
//! thin launcher driven by the editor). The launcher binary only starts the host;
//! the game library supplies its setup through the `fruits_entry_point!` macro.
//!
//! The launcher executable:
//!
//! ```ignore
//! fn main() {
//!     fruits_engine::launch_app_dynamically();
//! }
//! ```
//!
//! The game library exposes its world-building function to the launcher:
//!
//! ```ignore
//! use fruits_engine::WorldBuilderMut;
//!
//! fn setup(world: WorldBuilderMut) {
//!     // Register your components, systems, and resources on `world`.
//! }
//!
//! fruits_engine::fruits_entry_point!(setup);
//! ```
//!
//! # How to maintain
//!
//! Both entry points follow the same three steps: build an [`App`], register the
//! engine's default modules with [`fruits_modules::add_defult_modules_to`], let the
//! game populate the world, then hand control to [`App::run`]. They differ only in
//! *how* the game populates the world.
//!
//! [`launch_app_statically`] is the simple path: the caller's closure is invoked
//! directly with a [`WorldBuilderMut`] borrowed from the in-process [`App`].
//!
//! [`launch_app_dynamically`] crosses an FFI boundary. It looks for a shared library
//! named `lib_app` plus the platform's library suffix (`lib_app.dll`, `lib_app.so`, …)
//! next to the current executable and hands it to [`init_app_dynamically`]. That function
//! loads the library via `libloading`, resolves the `fruits_entry_point` C symbol (an
//! `unsafe extern "C-unwind" fn(AppInitCtxFfi)` that the game library exports through the
//! `fruits_entry_point!` macro), and exposes the world to it:
//! `WorldBuilderMut::into_raw_parts()` yields the raw world builder plus the type registry
//! cache, whose registry is taken; both are packed as raw pointers into an [`AppInitCtxFfi`]
//! and passed to the symbol, which reconstructs a safe `WorldBuilderMut` on the other side
//! and runs the game's setup. [`init_app_dynamically`] returns the loaded [`Library`] (or the
//! `libloading` error), and the library is closed only after [`App::run`] returns, because
//! the world holds types and function pointers that live inside it for the whole run.
//!
//! Caveats for maintainers: [`launch_app_dynamically`] `unwrap`s every fallible step
//! (locating the executable, loading `lib_app`, resolving the symbol, closing the
//! library), so a missing or mismatched library aborts the process rather than reporting
//! an error. The default-module
//! helper name `add_defult_modules_to` carries an upstream spelling and must match
//! `fruits_modules`.

use std::path::Path;

use fruits_app::App;
use fruits_ecs::{AppInitCtxFfi, WorldBuilderMut};
use libloading::Library;

pub fn launch_app_statically(f: impl FnOnce(WorldBuilderMut)) {
    let mut app = App::new();

    fruits_modules::add_defult_modules_to(app.ecs_mut().as_mut());

    f(app.ecs_mut().as_mut());

    app.run();
}

pub fn launch_app_dynamically() {
    let mut app = App::new();

    fruits_modules::add_defult_modules_to(app.ecs_mut().as_mut());

    let lib_path = std::env::current_exe().unwrap().with_file_name(format!("lib_app{}", std::env::consts::DLL_SUFFIX));
    let lib = unsafe { init_app_dynamically(app.ecs_mut().as_mut(), lib_path).unwrap() };

    app.run();

    lib.close().unwrap();
}

pub unsafe fn init_app_dynamically(mut world: WorldBuilderMut, lib_path: impl AsRef<Path>) -> Result<Library, libloading::Error> {
    let lib = unsafe { libloading::Library::new(lib_path.as_ref())? };
    let init_app_symbol = unsafe { lib.get::<unsafe extern "C-unwind" fn(AppInitCtxFfi)>(b"fruits_entry_point")? };

    unsafe {
        let (world_builder_ffi, types) = world.into_raw_parts();

        let types = types.registry();

        let ctx = AppInitCtxFfi {
            world_mut: &raw mut *world_builder_ffi,
            types_ref: &raw const *types,
        };

        init_app_symbol(ctx);
    }

    Ok(lib)
}