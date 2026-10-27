//! # aphyn_entry_point
//!
//! Connects a game built as a dynamic library to the engine launcher that loads
//! it, so the launcher can hand a freshly built world to the game's setup code.
//!
//! # How to use
//!
//! #### Export a setup function to the launcher
//!
//! Mark the function that populates the world so the launcher can find and call it
//! after loading the game library. The function takes an `AppInitCtx` (from
//! `aphyn_app`), registers the game's components, systems, and resources on
//! `ctx.world`, and may adjust the initial window through `ctx.window` (a
//! `WindowState`, applied when the launcher creates the window).
//!
//! ```ignore
//! use aphyn::*;
//!
//! fn setup(ctx: AppInitCtx) {
//!     // Register your components, systems, and resources on `ctx.world`.
//!     ctx.window.title = "My game".into();
//! }
//!
//! aphyn::aphyn_entry_point!(setup);
//! ```
//!
//! The launcher binary then loads the library and runs the host with
//! `aphyn::launch_app_dynamically()`. Only one `aphyn_entry_point!`
//! invocation belongs in a game library, since it defines the single exported
//! symbol the launcher looks up.
//!
//! # How to maintain
//!
//! The crate is a single function-like procedural macro. It takes the caller's
//! argument verbatim as the name of the setup function and emits a fixed
//! `#[unsafe(no_mangle)] pub unsafe extern "C-unwind" fn aphyn_entry_point` with C ABI.
//! That exported symbol is the contract with `aphyn_app_launcher`, which resolves
//! it by the literal name `aphyn_entry_point` and calls it with an
//! `AppInitCtxFfi`.
//!
//! The generated function only calls `AppInitCtxFfi::init_app` (in `aphyn_app`)
//! with the caller's setup function. That method dereferences the three raw pointers
//! (`world_mut`, `types_ref`, `window_mut`) back into references, wraps the type
//! registry in a `TypesRegistryCache`, rebuilds a safe `AppInitCtx` (a `WorldBuilderMut`
//! plus `&mut WindowState`), and invokes the setup function with it inside an inner
//! scope, so the borrows end before the function returns. `AppInitCtxFfi` is resolved
//! through `::aphyn` in the expanded code, so a game crate must depend on
//! `aphyn` for the macro output to compile.
//!
//! Caveats for maintainers: the macro performs no validation — it pastes the
//! argument string directly into a call expression, so a malformed argument
//! surfaces only as an error in the expanded code. The generated symbol name, ABI,
//! and `AppInitCtxFfi` layout must stay in lockstep with the lookup in
//! [`launch_app_dynamically`](https://docs.rs/aphyn_app_launcher); changing either
//! side alone breaks dynamic loading at runtime rather than at compile time.

extern crate proc_macro;
use proc_macro::TokenStream;

#[proc_macro]
pub fn aphyn_entry_point(item: TokenStream) -> TokenStream {
    let fn_name = item.to_string();

    format!(
        r#"
#[unsafe(no_mangle)]
pub unsafe extern "C-unwind" fn aphyn_entry_point(ctx: ::aphyn::AppInitCtxFfi) {{
    ctx.init_app({fn_name});
}}
    "#
    )
    .parse()
    .unwrap()
}
