//! # fruits_ecs_macros
//!
//! Derive macros that implement the `fruits_ecs` marker traits, so a plain data
//! type can be registered with the ECS world without writing the empty `impl` by
//! hand.
//!
//! # How to use
//!
//! These macros are re-exported from `fruits_ecs`, so depend on that crate and
//! derive the role you need; you never reference `fruits_ecs_macros` directly.
//! Each derive implements one marker trait whose own bounds (`Send + Sync`,
//! `Default`, ...) the type must still satisfy on its own.
//!
//! #### Tagging a struct as a component
//!
//! Use `Component` to let a type be attached to entities and queried in systems.
//!
//! ```ignore
//! use fruits_ecs::Component;
//!
//! #[derive(Component)]
//! struct Position {
//!     x: f32,
//!     y: f32,
//! }
//! ```
//!
//! #### Declaring a shared resource
//!
//! Use `Resource` for world-global state shared across systems. The trait
//! requires `Send + Sync`, which the type's fields must uphold.
//!
//! ```ignore
//! use fruits_ecs::Resource;
//!
//! #[derive(Resource, Default)]
//! struct FrameCounter {
//!     frames: u64,
//! }
//! ```
//!
//! #### Declaring an event type
//!
//! Use `Event` for messages written and read through the world's event channels.
//!
//! ```ignore
//! use fruits_ecs::Event;
//!
//! #[derive(Event)]
//! struct PlayerSpawned {
//!     entity_id: u32,
//! }
//! ```
//!
//! # How to maintain
//!
//! Each derive ([`derive_component`], [`derive_resource`], [`derive_event`])
//! emits the same shape of output: an empty `impl <Trait> for <Name> { }`. The
//! corresponding traits in `fruits_ecs` are markers with no methods, so no body
//! is generated; the derive only opts a type into the trait. Any extra bound the
//! trait carries (`Component: 'static + Send + Sync`,
//! `Resource: 'static + Send + Sync`, `Event: 'static`) is enforced by the
//! compiler against the user's type, not by the macro. Per-system state
//! (`fruits_ecs::Local`) needs no marker trait, so there is no derive for it.
//!
//! The type name is recovered by `get_struct_name`, a hand-rolled scan over the
//! input [`proc_macro::TokenStream`] rather than a `syn`-based parse (the crate
//! has no dependencies). It walks the token tree until it sees the `struct` or
//! `enum` keyword, then takes the next identifier as the type name. This means
//! both structs and enums are accepted, but the scan does **not** capture generic
//! parameters: deriving on a generic type would emit an `impl` without the
//! `<...>` parameters and fail to compile. If generic support is needed, this is
//! the function to change — most likely by switching to `syn`/`quote`. When the
//! name cannot be found the derive panics with a fixed message, surfacing as a
//! compile error at the derive site.

use proc_macro::TokenStream;

#[proc_macro_derive(Component)]
pub fn derive_component(stream: TokenStream) -> TokenStream {
    let Some(struct_name) = get_struct_name(stream) else {
        panic!("The name of the struct is not found.");
    };

    format!("impl Component for {struct_name} {{ }}").parse().unwrap()
}

#[proc_macro_derive(Resource)]
pub fn derive_resource(stream: TokenStream) -> TokenStream {
    let Some(struct_name) = get_struct_name(stream) else {
        panic!("The name of the struct is not found.");
    };

    format!("impl Resource for {struct_name} {{ }}").parse().unwrap()
}

#[proc_macro_derive(Event)]
pub fn derive_event(stream: TokenStream) -> TokenStream {
    let Some(struct_name) = get_struct_name(stream) else {
        panic!("The name of the struct is not found.");
    };

    format!("impl Event for {struct_name} {{ }}").parse().unwrap()
}

fn get_struct_name(stream: TokenStream) -> Option<String> {
    let mut iter = stream.into_iter();

    for tree in &mut iter {
        if let proc_macro::TokenTree::Ident(ident) = tree {
            if ident.to_string() == "struct" {
                break;
            }
            if ident.to_string() == "enum" {
                break;
            }
        }
    }

    let name_tree = iter.next()?;

    let proc_macro::TokenTree::Ident(name_ident) = name_tree else {
        return None;
    };

    Some(name_ident.to_string())
}
