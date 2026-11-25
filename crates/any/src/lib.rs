//! # aphyn_any
//!
//! Runtime type identity that works across the dynamic-library boundary, and type-erased values
//! built on it. [`TypesRegistryAccessFfi`] assigns every type a sequential `u64` id, keyed by its
//! `std::any::type_name`; [`AnyBox`], [`AnyRef`], [`AnyMut`] and [`AnyPtr`] carry that id next to
//! the value, so type checks compare ids instead of type names.
//!
//! # How to use
//!
//! #### Registering types
//!
//! A [`TypesRegistryCache`] wraps a registry and memoizes the `TypeId -> u64` lookup for the
//! current binary. [`RegistrySpecificTypeId<T>`] is the id of `T` in one registry; get it once
//! and keep it:
//!
//! ```
//! use aphyn_any::*;
//!
//! let types = TypesRegistryCache::new(TypesRegistryAccessFfi::new());
//! let id = types.type_id_of::<u32>();
//!
//! assert_eq!(types.get::<u32>(), Some(id.id()));
//! assert_eq!(id.tag(), types.tag());
//! ```
//!
//! #### Type-erased values
//!
//! [`AnyBox`] owns a value; [`AnyRef`], [`AnyMut`] and [`AnyPtr`] borrow one. Typed downcasts take
//! the expected type's [`RegistrySpecificTypeId`]:
//!
//! ```
//! use aphyn_any::*;
//!
//! let types = TypesRegistryCache::new(TypesRegistryAccessFfi::new());
//! let id_u32 = types.type_id_of::<u32>();
//! let id_f32 = types.type_id_of::<f32>();
//!
//! let mut value = AnyBox::new(5_u32, &types);
//! assert_eq!(value.downcast_ref(id_u32), Some(&5));
//! assert_eq!(value.downcast_ref(id_f32), None);
//!
//! *value.as_any_mut().downcast_mut(id_u32).unwrap() = 6;
//! assert_eq!(value.downcast(id_u32).ok(), Some(6));
//! ```
//!
//! [`AnyMut::replace`] moves a box of the same type into the borrowed place and returns the
//! previous value; a value of another type is handed back as `Err`:
//!
//! ```
//! use aphyn_any::*;
//!
//! let types = TypesRegistryCache::new(TypesRegistryAccessFfi::new());
//! let id = types.type_id_of::<String>();
//!
//! let mut name = String::from("old");
//! let mut place = AnyMut::new(&mut name, id);
//!
//! let previous = place.replace(AnyBox::new(String::from("new"), &types)).ok().unwrap();
//! assert_eq!(previous.downcast_ref(id).unwrap(), "old");
//! assert!(place.replace(AnyBox::new(5_u32, &types)).is_err());
//! assert_eq!(name, "new");
//! ```
//!
//! Type-erased code that already knows a value's id (a reflector or serializer resolved it when
//! its registry was built) builds anys with the `unsafe` `from_typed` / `from_raw` constructors and
//! an [`AnyBoxCtx`] (the id plus the registry a box keeps alive).
//!
//! # How to maintain
//!
//! #### The registry
//!
//! Types are not identified by [`std::any::TypeId`] across the boundary: it is not stable between
//! separately compiled binaries. [`TypesRegistryAccessFfi`] assigns ids keyed by `type_name`, so
//! the same type gets the same id on both sides, and stores its `FfiExtendedTypeInfo` (size,
//! alignment, name, drop and box functions). It is a refcounted handle: its data pointer is
//! `Arc::into_raw` of the shared state, and the vtable's clone and drop increment and decrement
//! the strong count in the binary that created it, so a clone costs one atomic increment. Each
//! `WorldData` creates its own registry, and its ids are only meaningful inside it.
//! [`TypesRegistryCache`] memoizes `TypeId -> u64` per binary and per registry; a cache is only
//! valid for the registry it was built from. `aphyn_ecs` re-exports this module, and its
//! containers register components, resources, events and system resources lazily through
//! `get_or_register` the first time they are used.
//!
//! #### Tags and ids
//!
//! A [`TypesRegistryTag`] is the address of a registry's shared state. [`AnyTypeId`] pairs a tag
//! with an id, and [`RegistrySpecificTypeId`] wraps one. Two anys (or an any and an expected
//! type) compare ids only when their tags are equal and set; otherwise they fall back to
//! comparing size, align and type name ([`AnyTypeId::matches`]). The fallback is what makes
//! values safe to move between worlds: the editor keeps values cloned from a simulated world in
//! its own world. [`AnyTypeId::UNRESOLVED`] has no tag and always takes the fallback.
//!
//! A tag must not be reused while an any carrying it exists. [`AnyBox`] therefore holds a
//! refcounted [`TypesRegistryAccessFfi`] of its registry, which keeps the state, and so its
//! address, alive for as long as the box. [`AnyRef`], [`AnyMut`] and [`AnyPtr`] store only the
//! tag: they borrow data owned by a world or a box, which already keeps the registry alive.
//!
//! #### Construction and memory
//!
//! Generic constructors take a [`RegistrySpecificTypeId`] (or the cache) so the id is correct by
//! construction. The `unsafe` `from_typed` / `from_raw` / `from_moved` constructors trust the
//! caller's id; a wrong id with a set tag makes later downcasts reinterpret memory as the wrong
//! type, which is why they are `unsafe`. An [`AnyBox`] is allocated either by `FfiBox::new` (the
//! generic path) or by `FfiExtendedTypeInfo::move_to_box`, and freed through the type info's
//! `dealloc_box`. Both are monomorphized in the binary that built the type info, so the memory is
//! always freed by the allocator that allocated it.

mod any;
mod types_registry;

pub use any::*;
pub use types_registry::*;

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;

    #[test]
    fn same_registry_downcasts_by_id() {
        let types = TypesRegistryCache::new(TypesRegistryAccessFfi::new());
        let value = AnyBox::new(3_u64, &types);

        assert_eq!(value.type_id().tag(), types.tag());
        assert_eq!(value.downcast_ref(types.type_id_of::<u64>()), Some(&3));
        assert_eq!(value.downcast_ref(types.type_id_of::<i64>()), None);
    }

    #[test]
    fn other_registry_falls_back_to_type_info() {
        let a = TypesRegistryCache::new(TypesRegistryAccessFfi::new());
        let b = TypesRegistryCache::new(TypesRegistryAccessFfi::new());

        // different registration order, so the same id means different types in each registry
        a.get_or_register::<u8>();
        let value = AnyBox::new(String::from("x"), &a);

        assert_ne!(a.tag(), b.tag());
        assert_eq!(b.type_id_of::<String>().id(), 0);
        assert_eq!(value.downcast_ref(b.type_id_of::<String>()).map(String::as_str), Some("x"));
        assert_eq!(value.downcast_ref(b.type_id_of::<u8>()), None);
    }

    #[test]
    fn replace_across_registries() {
        let a = TypesRegistryCache::new(TypesRegistryAccessFfi::new());
        let b = TypesRegistryCache::new(TypesRegistryAccessFfi::new());
        b.get_or_register::<u8>();

        let mut place = 1_u32;
        let mut place = AnyMut::new(&mut place, a.type_id_of::<u32>());

        let previous = place.replace(AnyBox::new(2_u32, &b)).ok().unwrap();
        assert_eq!(previous.downcast_ref(b.type_id_of::<u32>()), Some(&1));
        assert!(place.replace(AnyBox::new(2_u8, &b)).is_err());
    }

    #[test]
    fn unresolved_ids_compare_by_type_info() {
        let types = TypesRegistryCache::new(TypesRegistryAccessFfi::new());
        let value = unsafe { AnyBox::from_typed(7_i32, AnyBoxCtx::UNRESOLVED) };

        assert!(!value.type_id().is_resolved());
        assert_eq!(value.downcast_ref(types.type_id_of::<i32>()), Some(&7));
        assert_eq!(value.downcast_ref(types.type_id_of::<u32>()), None);
    }

    #[test]
    fn box_keeps_registry_alive() {
        let types = TypesRegistryCache::new(TypesRegistryAccessFfi::new());
        let tag = types.tag();
        let value = AnyBox::new(Arc::new(5_u8), &types);

        drop(types);

        assert_eq!(value.registry().map(|r| r.tag()), Some(tag));
        assert!(value.registry().unwrap().len() >= 1);
    }

    #[test]
    fn zero_sized_values_box_and_drop() {
        #[derive(Debug, PartialEq)]
        struct Marker;

        let types = TypesRegistryCache::new(TypesRegistryAccessFfi::new());
        let value = AnyBox::new(Marker, &types);

        assert_eq!(value.downcast(types.type_id_of::<Marker>()).ok(), Some(Marker));
    }
}
