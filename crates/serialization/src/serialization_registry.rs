use std::{ffi::c_void, marker::PhantomData, mem::MaybeUninit};

use aphyn_any::{AnyBox, AnyBoxCtx, AnyMut, AnyRef, AnyTypeId, TypesRegistryAccessFfi, TypesRegistryCache};
use aphyn_ffi::{FfiArc, FfiDroppable, FfiExtendedTypeInfo, FfiIndexMap, FfiOption, FfiStrSliceRef, FfiString, FfiVec};

use crate::{ContextData, Serializable, SerializationError, SerializedValue, SerializerCtx, TransSerializerCtxState};

// todo: the order of arguments in all of the serialization: self, ctx, Deserialized, path, serialized

pub trait Serializer<S: Copy> {
    type Deserialized;

    fn serializable_default(&self) -> Self::Deserialized;
    fn serialize(&self, value: &Self::Deserialized, ctx: SerializerCtx<S>, path: &str) -> SerializedValue;
    fn deserialize(&self, value: &mut Self::Deserialized, ctx: SerializerCtx<S>, path: &str, serialized: &SerializedValue);
}

#[repr(C)]
pub struct StandardSerializer<T> {
    _phantom: PhantomData<fn(T) -> T>,
    fn_default: fn() -> T,
}

impl<T> StandardSerializer<T> {
    pub fn new(fn_default: fn() -> T) -> Self {
        Self {
            fn_default,
            _phantom: Default::default(),
        }
    }
}

impl<T: Default> Default for StandardSerializer<T> {
    fn default() -> Self {
        Self::new(|| Default::default())
    }
}

impl<S: Copy, T: Serializable<S>> Serializer<S> for StandardSerializer<T> {
    type Deserialized = T;

    fn serializable_default(&self) -> Self::Deserialized {
        (self.fn_default)()
    }

    fn serialize(&self, value: &Self::Deserialized, ctx: SerializerCtx<S>, path: &str) -> SerializedValue {
        value.serialize(ctx, path)
    }

    fn deserialize(&self, value: &mut Self::Deserialized, ctx: SerializerCtx<S>, path: &str, serialized: &SerializedValue) {
        value.deserialize(ctx, path, serialized);
    }
}

//

#[repr(C)]
struct TransSerializerFfiVtable {
    fn_serializable_default: unsafe extern "C-unwind" fn(*const c_void, out: *mut c_void),
    fn_serializable_default_any: unsafe extern "C-unwind" fn(*const c_void, ctx: AnyBoxCtx) -> AnyBox,
    fn_serialize: unsafe extern "C-unwind" fn(
        *const c_void,
        local_types: *const c_void,
        value: *const c_void,
        ctx: SerializerCtx<TransSerializerCtxState>,
        path: FfiStrSliceRef,
    ) -> SerializedValue,
    fn_deserialize: unsafe extern "C-unwind" fn(
        *const c_void,
        local_types: *const c_void,
        value: *mut c_void,
        ctx: SerializerCtx<TransSerializerCtxState>,
        path: FfiStrSliceRef,
        serialized: &SerializedValue,
    ),
}

#[repr(C)]
pub struct TransSerializerFfi {
    data: FfiDroppable,
    // a `TypesRegistryCache` of the binary that registered the serializer; only its own trampolines read it
    local_types: FfiDroppable,
    vtable: &'static TransSerializerFfiVtable,
    deserialized_type_info: &'static FfiExtendedTypeInfo,
    deserialized_id: AnyTypeId,
}

impl TransSerializerFfi {
    fn new<T: 'static, S: 'static + for<'ctx> Serializer<TransSerializerCtxState<'ctx>, Deserialized = T> + Send + Sync>(
        types: TypesRegistryCache,
        serializer: S,
    ) -> Self {
        unsafe extern "C-unwind" fn ffi_serializable_default<
            T,
            S: 'static + for<'ctx> Serializer<TransSerializerCtxState<'ctx>, Deserialized = T> + Send + Sync,
        >(
            this: *const c_void,
            out: *mut c_void,
        ) {
            unsafe {
                let serializer = &*(this as *const S);
                let out = out as *mut T;

                let result = serializer.serializable_default();

                out.write(result);
            }
        }
        unsafe extern "C-unwind" fn ffi_serializable_default_any<
            T: 'static,
            S: 'static + for<'ctx> Serializer<TransSerializerCtxState<'ctx>, Deserialized = T> + Send + Sync,
        >(
            this: *const c_void,
            ctx: AnyBoxCtx,
        ) -> AnyBox {
            unsafe {
                let serializer = &*(this as *const S);

                let result = serializer.serializable_default();

                AnyBox::from_typed(result, ctx)
            }
        }
        unsafe extern "C-unwind" fn ffi_serialize<
            T,
            S: 'static + for<'ctx> Serializer<TransSerializerCtxState<'ctx>, Deserialized = T> + Send + Sync,
        >(
            this: *const c_void,
            local_types: *const c_void,
            value: *const c_void,
            ctx: SerializerCtx<TransSerializerCtxState>,
            path: FfiStrSliceRef,
        ) -> SerializedValue {
            unsafe {
                let serializer = &*(this as *const S);
                let local_types = &*(local_types as *const TypesRegistryCache);
                let value = &*(value as *const T);
                let path = path.into_slice();

                serializer.serialize(value, ctx.map_state(|s| s.with_local_types(local_types)), path)
            }
        }
        unsafe extern "C-unwind" fn ffi_deserialize<
            T,
            S: 'static + for<'ctx> Serializer<TransSerializerCtxState<'ctx>, Deserialized = T> + Send + Sync,
        >(
            this: *const c_void,
            local_types: *const c_void,
            value: *mut c_void,
            ctx: SerializerCtx<TransSerializerCtxState>,
            path: FfiStrSliceRef,
            serialized: &SerializedValue,
        ) {
            unsafe {
                let serializer = &*(this as *const S);
                let local_types = &*(local_types as *const TypesRegistryCache);
                let value = &mut *(value as *mut T);
                let path = path.into_slice();

                serializer.deserialize(value, ctx.map_state(|s| s.with_local_types(local_types)), path, serialized);
            }
        }

        let deserialized_type_info = const { &FfiExtendedTypeInfo::of::<T>() };

        Self {
            data: FfiDroppable::new(serializer),
            deserialized_id: types.any_type_id(deserialized_type_info),
            deserialized_type_info,
            local_types: FfiDroppable::new(types),
            vtable: &TransSerializerFfiVtable {
                fn_serializable_default: ffi_serializable_default::<T, S>,
                fn_serializable_default_any: ffi_serializable_default_any::<T, S>,
                fn_serialize: ffi_serialize::<T, S>,
                fn_deserialize: ffi_deserialize::<T, S>,
            },
        }
    }

    pub fn deserialized_type_id(&self) -> AnyTypeId {
        self.deserialized_id
    }

    pub fn deserialized_type_info(&self) -> &'static FfiExtendedTypeInfo {
        self.deserialized_type_info
    }

    // safety: `T` is the deserialized type
    unsafe fn serializable_default<T>(&self) -> T {
        unsafe {
            let mut out = MaybeUninit::<T>::uninit();

            (self.vtable.fn_serializable_default)(self.data.get(), out.as_mut_ptr() as *mut c_void);

            out.assume_init()
        }
    }

    // safety: `registry` is the one `deserialized_type_id` was resolved in
    pub unsafe fn serializable_default_any(&self, registry: Option<&TypesRegistryAccessFfi>) -> AnyBox {
        let ctx = AnyBoxCtx {
            id: self.deserialized_id,
            registry: registry.into(),
        };

        unsafe { (self.vtable.fn_serializable_default_any)(self.data.get(), ctx) }
    }

    // safety: `value` points to a value of the deserialized type
    unsafe fn serialize_ptr(
        &self,
        value: *const c_void,
        ctx: SerializerCtx<TransSerializerCtxState>,
        path: &str,
    ) -> SerializedValue {
        unsafe {
            let path = FfiStrSliceRef::from_slice(path);

            (self.vtable.fn_serialize)(self.data.get(), self.local_types.get(), value, ctx, path)
        }
    }

    // safety: `value` points to a value of the deserialized type
    unsafe fn deserialize_ptr(
        &self,
        value: *mut c_void,
        ctx: SerializerCtx<TransSerializerCtxState>,
        path: &str,
        serialized: &SerializedValue,
    ) {
        unsafe {
            let path = FfiStrSliceRef::from_slice(path);

            (self.vtable.fn_deserialize)(self.data.get(), self.local_types.get(), value, ctx, path, serialized);
        }
    }

    // safety: `value` is of the deserialized type
    pub unsafe fn serialize_any(&self, value: AnyRef, ctx: SerializerCtx<TransSerializerCtxState>, path: &str) -> SerializedValue {
        unsafe { self.serialize_ptr(value.ptr(), ctx, path) }
    }

    // safety: `value` is of the deserialized type
    pub unsafe fn deserialize_any(
        &self,
        value: AnyMut,
        ctx: SerializerCtx<TransSerializerCtxState>,
        path: &str,
        serialized: &SerializedValue,
    ) {
        unsafe { self.deserialize_ptr(value.ptr(), ctx, path, serialized) }
    }
}

unsafe impl Send for TransSerializerFfi {}
unsafe impl Sync for TransSerializerFfi {}

pub struct TransSerializerFfiTypedRef<'r, T> {
    serializer: &'r TransSerializerFfi,
    _phantom: PhantomData<fn(T) -> T>,
}

impl<'r, T> TransSerializerFfiTypedRef<'r, T> {
    // safety: `T` is the serializer's deserialized type
    unsafe fn new(serializer: &'r TransSerializerFfi) -> Self {
        Self {
            serializer,
            _phantom: PhantomData,
        }
    }

    pub fn serializable_default(&self) -> T {
        unsafe { self.serializer.serializable_default() }
    }

    pub fn serialize(&self, value: &T, ctx: SerializerCtx<TransSerializerCtxState>, path: &str) -> SerializedValue {
        unsafe { self.serializer.serialize_ptr(value as *const T as *const c_void, ctx, path) }
    }

    pub fn deserialize(&self, value: &mut T, ctx: SerializerCtx<TransSerializerCtxState>, path: &str, serialized: &SerializedValue) {
        unsafe {
            self.serializer
                .deserialize_ptr(value as *mut T as *mut c_void, ctx, path, serialized)
        }
    }
}

//

#[repr(C)]
pub struct TransSerializerEntry {
    name: FfiString,
    serializer: TransSerializerFfi,
}

impl TransSerializerEntry {
    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn type_id(&self) -> AnyTypeId {
        self.serializer.deserialized_id
    }

    pub fn serializer(&self) -> &TransSerializerFfi {
        &self.serializer
    }
}

#[repr(C)]
#[derive(Default)]
struct TransSerializerTable {
    entries: FfiVec<TransSerializerEntry>,
    // indexed by the deserialized type's id in the registry's types registry
    by_type: FfiVec<FfiOption<u64>>,
    by_name: FfiIndexMap<FfiString, u64>,
}

impl TransSerializerTable {
    fn by_id(&self, id: u64) -> Option<&TransSerializerEntry> {
        let idx = *self.by_type.get(id)?.as_ref()?;
        self.entries.get(idx)
    }

    fn by_name(&self, name: &str) -> Option<&TransSerializerEntry> {
        self.entries.get(*self.by_name.get(name)?)
    }
}

#[repr(C)]
#[derive(Default)]
pub struct TransSerializerRegistryBuilder {
    table: TransSerializerTable,
    types: FfiOption<TypesRegistryAccessFfi>,
}

impl TransSerializerRegistryBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    // `types` is the calling binary's cache of the registry of the world the values live in (one registry per
    // builder); `name` identifies the type in serialized data. Registering a type again replaces its serializer
    // and must repeat its name; a name can't be used for two types.
    pub fn register<T: 'static>(
        &mut self,
        types: &TypesRegistryCache,
        name: &str,
        serializer: impl 'static + for<'ctx> Serializer<TransSerializerCtxState<'ctx>, Deserialized = T> + Send + Sync,
    ) -> &mut Self {
        self.check_types(types);

        let serializer = TransSerializerFfi::new(types.clone(), serializer);
        let id = serializer.deserialized_id.id();
        let table = &mut self.table;

        if let Some(FfiOption::Some(idx)) = table.by_type.get(id).cloned() {
            let entry = table.entries.get_mut(idx).unwrap();
            assert!(
                entry.name.as_str() == name,
                "the serializer for {} is registered as `{}` and `{name}`",
                serializer.deserialized_type_info.short().name(),
                entry.name.as_str(),
            );
            entry.serializer = serializer;
            return self;
        }

        assert!(
            !table.by_name.contains_key(name),
            "the serialized name `{name}` is used by two types (the second is {})",
            serializer.deserialized_type_info.short().name(),
        );

        let idx = table.entries.len();
        table.entries.push(TransSerializerEntry {
            name: name.into(),
            serializer,
        });
        if table.by_type.len() <= id {
            table.by_type.resize(id + 1, FfiOption::None);
        }
        *table.by_type.get_mut(id).unwrap() = FfiOption::Some(idx);
        table.by_name.insert(name.into(), idx);
        self
    }

    pub fn contains(&self, id: AnyTypeId) -> bool {
        self.table.by_id(id.id()).is_some()
    }

    pub fn contains_name(&self, name: &str) -> bool {
        self.table.by_name.contains_key(name)
    }

    pub fn build(self) -> TransSerializerRegistry {
        TransSerializerRegistry {
            table: FfiArc::new(self.table),
            types: self.types,
        }
    }

    fn check_types(&mut self, types: &TypesRegistryCache) {
        match self.types.as_ref() {
            Some(registered) => assert!(
                registered.tag() == types.tag(),
                "all serializer registrations must use the same types registry"
            ),
            None => self.types = FfiOption::Some(types.ffi().clone()),
        }
    }
}

#[repr(C)]
#[derive(Clone)]
pub struct TransSerializerRegistry {
    table: FfiArc<TransSerializerTable>,
    types: FfiOption<TypesRegistryAccessFfi>,
}

impl Default for TransSerializerRegistry {
    fn default() -> Self {
        TransSerializerRegistryBuilder::new().build()
    }
}

impl TransSerializerRegistry {
    // the types registry the serializers' ids were resolved in; None when nothing was registered
    pub fn types(&self) -> Option<&TypesRegistryAccessFfi> {
        self.types.as_ref()
    }

    pub fn serialize<'r, T: 'static>(
        &'r self,
        local_types: &'r TypesRegistryCache,
        value: &T,
        path: &str,
        err_handler: &'r mut impl FnMut(SerializationError),
    ) -> SerializedValue {
        self.to_ctx(&ContextData::new(), local_types, err_handler)
            .serialize(value, path)
    }

    pub fn deserialize<'r, T: 'static>(
        &'r self,
        local_types: &'r TypesRegistryCache,
        value: &mut T,
        path: &str,
        serialized: &SerializedValue,
        err_handler: &'r mut impl FnMut(SerializationError),
    ) {
        self.to_ctx(&ContextData::new(), local_types, err_handler)
            .deserialize(value, path, serialized)
    }

    pub fn entries(&self) -> impl Iterator<Item = &TransSerializerEntry> {
        self.table.entries.as_slice().iter()
    }

    pub fn get_by_id(&self, id: AnyTypeId) -> Option<&TransSerializerEntry> {
        if !self.is_own(id) {
            return None;
        }
        self.table.by_id(id.id())
    }

    pub fn get_by_name(&self, name: &str) -> Option<&TransSerializerEntry> {
        self.table.by_name(name)
    }

    pub fn name_of(&self, id: AnyTypeId) -> Option<&str> {
        self.get_by_id(id).map(TransSerializerEntry::name)
    }

    pub fn type_id_of_name(&self, name: &str) -> Option<AnyTypeId> {
        self.get_by_name(name).map(TransSerializerEntry::type_id)
    }

    // the id of a value's type in this registry's types registry, also for values of another world
    pub fn own_type_id(&self, id: AnyTypeId, info: &'static FfiExtendedTypeInfo) -> Option<AnyTypeId> {
        if self.is_own(id) {
            return Some(id);
        }
        let types = self.types.as_ref()?;
        Some(AnyTypeId::new(types.tag(), types.get_by_name(info.short().name())?.id))
    }

    pub fn get_for(&self, id: AnyTypeId, info: &'static FfiExtendedTypeInfo) -> Option<&TransSerializerEntry> {
        self.get_by_id(self.own_type_id(id, info)?)
    }

    pub(crate) fn get<'r, T: 'static>(&'r self, local_types: &TypesRegistryCache) -> Option<TransSerializerFfiTypedRef<'r, T>> {
        let entry = self.table.by_id(local_types.get_or_register::<T>())?;
        unsafe { Some(TransSerializerFfiTypedRef::new(&entry.serializer)) }
    }

    fn is_own(&self, id: AnyTypeId) -> bool {
        self.types.as_ref().is_some_and(|t| t.tag() == id.tag())
    }

    // `local_types` is the calling binary's cache of the registry the serializers were registered with
    pub fn to_ctx_state<'a>(
        &'a self,
        context: &'a ContextData<'a>,
        local_types: &'a TypesRegistryCache,
    ) -> TransSerializerCtxState<'a> {
        TransSerializerCtxState::new(self, context, local_types)
    }

    pub fn to_ctx<'r, 'h>(
        &'r self,
        context: &'r ContextData<'r>,
        local_types: &'r TypesRegistryCache,
        err_handler: &'h mut impl FnMut(SerializationError),
    ) -> SerializerCtx<'h, TransSerializerCtxState<'r>> {
        SerializerCtx::new(self.to_ctx_state(context, local_types), err_handler)
    }
}

#[cfg(test)]
mod tests {
    use aphyn_any::{AnyRef, TypesRegistryAccessFfi, TypesRegistryCache};

    use crate::{
        ContextData, SerializationError, SerializedPrimitive, SerializedValue, StandardSerializer, TransSerializerRegistryBuilder,
    };

    fn new_types() -> TypesRegistryCache {
        TypesRegistryCache::new(TypesRegistryAccessFfi::new())
    }

    #[test]
    #[should_panic(expected = "same types registry")]
    fn registering_with_two_registries_panics() {
        let mut builder = TransSerializerRegistryBuilder::new();
        builder.register(&new_types(), "u32", StandardSerializer::<u32>::default());
        builder.register(&new_types(), "f32", StandardSerializer::<f32>::default());
    }

    #[test]
    #[should_panic(expected = "is registered as")]
    fn renaming_a_type_panics() {
        let types = new_types();
        let mut builder = TransSerializerRegistryBuilder::new();
        builder.register(&types, "u32", StandardSerializer::<u32>::default());
        builder.register(&types, "uint", StandardSerializer::<u32>::default());
    }

    #[test]
    #[should_panic(expected = "used by two types")]
    fn sharing_a_name_panics() {
        let types = new_types();
        let mut builder = TransSerializerRegistryBuilder::new();
        builder.register(&types, "number", StandardSerializer::<u32>::default());
        builder.register(&types, "number", StandardSerializer::<f32>::default());
    }

    #[test]
    fn lookups_by_id_and_name_agree() {
        let types = new_types();
        let mut builder = TransSerializerRegistryBuilder::new();
        builder
            .register(&types, "u32", StandardSerializer::<u32>::default())
            .register(&types, "u32", StandardSerializer::<u32>::default());
        let registry = builder.build();

        let id = types.type_id_of::<u32>().any_type_id();
        assert_eq!(registry.entries().count(), 1);
        assert_eq!(registry.name_of(id), Some("u32"));
        assert_eq!(registry.type_id_of_name("u32"), Some(id));

        let mut on_err = |err: SerializationError| panic!("{err}");
        let serialized = registry.serialize(&types, &7_u32, "", &mut on_err);
        let data = ContextData::new();
        let mut ctx = registry.to_ctx(&data, &types, &mut on_err);
        let by_name = ctx.deserialize_default_any("u32", "", &serialized).unwrap();
        let by_id = ctx.deserialize_default_any_by_id(id, "", &serialized).unwrap();
        assert_eq!(by_name.downcast_ref(types.type_id_of::<u32>()), Some(&7));
        assert_eq!(by_id.downcast_ref(types.type_id_of::<u32>()), Some(&7));
    }

    #[test]
    fn values_of_another_world_find_their_serializer() {
        let types = new_types();
        let other = new_types();
        other.get_or_register::<u8>();
        let mut builder = TransSerializerRegistryBuilder::new();
        builder.register(&types, "f32", StandardSerializer::<f32>::default());
        let registry = builder.build();

        let value = 1.5_f32;
        let foreign = AnyRef::new(&value, other.type_id_of());
        let mut on_err = |err: SerializationError| panic!("{err}");
        let data = ContextData::new();
        let serialized = registry.to_ctx(&data, &types, &mut on_err).serialize_any(foreign, "");
        assert!(matches!(serialized, SerializedValue::Primitive(SerializedPrimitive::Float(v)) if v == 1.5));
    }
}
