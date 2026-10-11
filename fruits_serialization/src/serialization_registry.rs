use std::{ffi::c_void, marker::PhantomData, mem::MaybeUninit};

use fruits_ffi::{FfiAny, FfiAnyMut, FfiAnyRef, FfiDroppable, FfiIndexMap, FfiStrSliceRef, FfiString};

use crate::{
    Serializable, SerializationError, SerializedValue, SerializerCtx, TransSerializerCtxState,
};

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
    fn_serializable_default_any: unsafe extern "C-unwind" fn(*const c_void) -> FfiAny,
    fn_serialize: unsafe extern "C-unwind" fn(
        *const c_void,
        value: *const c_void,
        ctx: SerializerCtx<TransSerializerCtxState>,
        path: FfiStrSliceRef,
    ) -> SerializedValue,
    fn_deserialize: unsafe extern "C-unwind" fn(
        *const c_void,
        value: *mut c_void,
        ctx: SerializerCtx<TransSerializerCtxState>,
        path: FfiStrSliceRef,
        serialized: &SerializedValue,
    ),
}

#[repr(C)]
pub struct TransSerializerFfi<'se> {
    data: FfiDroppable,
    vtable: &'static TransSerializerFfiVtable,
    _phantom: PhantomData<&'se mut ()>,
}

impl<'se> TransSerializerFfi<'se> {
    fn new<T: 'static, S: 'se + for<'ctx> Serializer<TransSerializerCtxState<'ctx>, Deserialized = T> + Send + Sync>(serializer: S) -> Self {
        unsafe extern "C-unwind" fn ffi_serializable_default<'se, T, S: 'se + for<'ctx> Serializer<TransSerializerCtxState<'ctx>, Deserialized = T> + Send + Sync>(
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
        unsafe extern "C-unwind" fn ffi_serializable_default_any<'se, T: 'static, S: 'se + for<'ctx> Serializer<TransSerializerCtxState<'ctx>, Deserialized = T> + Send + Sync>(
            this: *const c_void,
        ) -> FfiAny {
            unsafe {
                let serializer = &*(this as *const S);

                let result = serializer.serializable_default();

                FfiAny::new(result)
            }
        }
        unsafe extern "C-unwind" fn ffi_serialize<'se, T, S: 'se + for<'ctx> Serializer<TransSerializerCtxState<'ctx>, Deserialized = T> + Send + Sync>(
            this: *const c_void,
            value: *const c_void,
            ctx: SerializerCtx<TransSerializerCtxState>,
            path: FfiStrSliceRef,
        ) -> SerializedValue {
            unsafe {
                let serializer = &*(this as *const S);
                let value = &*(value as *const T);
                let path = path.into_slice();

                serializer.serialize(value, ctx, path)
            }
        }
        unsafe extern "C-unwind" fn ffi_deserialize<'se, T, S: 'se + for<'ctx> Serializer<TransSerializerCtxState<'ctx>, Deserialized = T> + Send + Sync>(
            this: *const c_void,
            value: *mut c_void,
            ctx: SerializerCtx<TransSerializerCtxState>,
            path: FfiStrSliceRef,
            serialized: &SerializedValue,
        ) {
            unsafe {
                let serializer = &*(this as *const S);
                let value = &mut *(value as *mut T);
                let path = path.into_slice();

                serializer.deserialize(value, ctx, path, serialized);
            }
        }

        Self {
            data: FfiDroppable::new(serializer),
            vtable: &TransSerializerFfiVtable {
                fn_serializable_default: ffi_serializable_default::<T, S>,
                fn_serializable_default_any: ffi_serializable_default_any::<T, S>,
                fn_serialize: ffi_serialize::<T, S>,
                fn_deserialize: ffi_deserialize::<T, S>,
            },
            _phantom: PhantomData,
        }
    }

    // todo
    unsafe fn serializable_default<T>(&self) -> T {
        unsafe {
            let mut out = MaybeUninit::<T>::uninit();

            (self.vtable.fn_serializable_default)(self.data.get(), out.as_mut_ptr() as *mut c_void);

            out.assume_init()
        }
    }
    // todo
    pub unsafe fn serializable_default_any(&self) -> FfiAny {
        unsafe {
            (self.vtable.fn_serializable_default_any)(self.data.get())
        }
    }
    // todo
    unsafe fn serialize<T>(&self, value: &T, ctx: SerializerCtx<TransSerializerCtxState>, path: &str) -> SerializedValue {
        unsafe {
            let value = value as *const T as *const c_void;
            let path = FfiStrSliceRef::from_slice(path);

            (self.vtable.fn_serialize)(self.data.get(), value, ctx, path)
        }
    }
    // todo
    pub unsafe fn serialize_any(&self, value: FfiAnyRef, ctx: SerializerCtx<TransSerializerCtxState>, path: &str) -> SerializedValue {
        unsafe {
            let value = value.ptr();
            let path = FfiStrSliceRef::from_slice(path);

            (self.vtable.fn_serialize)(self.data.get(), value, ctx, path)
        }
    }
    // todo
    unsafe fn deserialize<T>(&self, value: &mut T, ctx: SerializerCtx<TransSerializerCtxState>, path: &str, serialized: &SerializedValue) {
        unsafe {
            let value = value as *mut T as *mut c_void;
            let path = FfiStrSliceRef::from_slice(path);

            (self.vtable.fn_deserialize)(self.data.get(), value, ctx, path, serialized);
        }
    }
    //
    // todo
    // fn deserialized_type_name(&self) -> &'static str {
    //     std::any::type_name::<T>()
    // }
    pub unsafe fn deserialize_any(&self, value: FfiAnyMut, ctx: SerializerCtx<TransSerializerCtxState>, path: &str, serialized: &SerializedValue) {
        unsafe {
            let value = value.ptr();
            let path = FfiStrSliceRef::from_slice(path);

            (self.vtable.fn_deserialize)(self.data.get(), value, ctx, path, serialized);
        }
    }
}

unsafe impl<'se> Send for TransSerializerFfi<'se> {}
unsafe impl<'se> Sync for TransSerializerFfi<'se> {}

pub struct TransSerializerFfiTypedRef<'r, 'se, T> {
    serializer: &'r TransSerializerFfi<'se>,
    _phantom: PhantomData<fn(T) -> T>,
}

impl<'r, 'se, T> TransSerializerFfiTypedRef<'r, 'se, T> {
    unsafe fn new(serializer: &'r TransSerializerFfi<'se>) -> Self {
        Self {
            serializer,
            _phantom: PhantomData,
        }
    }

    pub fn serializable_default(&self) -> T {
        // todo
        unsafe { self.serializer.serializable_default() }
    }

    pub fn serialize(&self, value: &T, ctx: SerializerCtx<TransSerializerCtxState>, path: &str) -> SerializedValue {
        // todo
        unsafe { self.serializer.serialize::<T>(value, ctx, path) }
    }

    pub unsafe fn serialize_any(&self, value: FfiAnyRef, ctx: SerializerCtx<TransSerializerCtxState>, path: &str) -> SerializedValue {
        // todo
        unsafe { self.serializer.serialize_any(value, ctx, path) }
    }

    pub fn deserialize(&self, value: &mut T, ctx: SerializerCtx<TransSerializerCtxState>, path: &str, serialized: &SerializedValue) {
        // todo
        unsafe { self.serializer.deserialize::<T>(value, ctx, path, serialized) }
    }

    pub fn deserialize_any(&self, value: FfiAnyMut, ctx: SerializerCtx<TransSerializerCtxState>, path: &str, serialized: &SerializedValue) {
        // todo
        unsafe {
            self.serializer.deserialize_any(value, ctx, path, serialized);
        }
    }
}

//

#[repr(C)]
#[derive(Default)]
pub struct TransSerializerRegistry<'se> {
    serializers: FfiIndexMap<FfiString, TransSerializerFfi<'se>>,
}

impl<'se> TransSerializerRegistry<'se> {
    pub fn new() -> Self {
        Self {
            serializers: FfiIndexMap::new(),
        }
    }

    pub fn register<T: 'static>(
        &mut self,
        serializer: impl 'se + for<'ctx> Serializer<TransSerializerCtxState<'ctx>, Deserialized = T> + Send + Sync,
    ) {
        let serializer = TransSerializerFfi::new(serializer);

        let type_name = std::any::type_name::<T>().into();

        self.serializers.insert(type_name, serializer);
    }

    pub fn serialize<'r, T: 'static>(
        &'r mut self,
        value: &T,
        path: &str,
        err_handler: &'r mut impl FnMut(SerializationError),
    ) -> SerializedValue {
        self.to_ctx(err_handler).serialize(value, path)
    }

    pub fn deserialize<'r, T: 'static>(
        &'r mut self,
        value: &mut T,
        path: &str,
        serialized: &SerializedValue,
        err_handler: &'r mut impl FnMut(SerializationError),
    ) {
        self.to_ctx(err_handler).deserialize(value, path, serialized)
    }

    pub fn keys(&self) -> impl Iterator<Item = &str> {
        self.serializers.keys().map(|s| s.as_str())
    }

    pub(crate) fn get<'r, T: 'static>(&'r self) -> Option<TransSerializerFfiTypedRef<'r, 'se, T>>
    where
        'se: 'r,
    {
        unsafe {
            let type_name = std::any::type_name::<T>();
            let serializer = self.serializers.get(type_name)?;
            Some(TransSerializerFfiTypedRef::new(serializer))
        }
    }

    pub(crate) fn get_virtual<'r>(&'r self, id: &str) -> Option<&'r TransSerializerFfi<'se>>
    where
        'se: 'r,
    {
        self.serializers.get(id)
    }
    
    pub fn to_ctx_state<'a>(&'a self) -> TransSerializerCtxState<'a> {
        TransSerializerCtxState::new(self, None)
    }

    pub fn to_ctx<'r>(
        &'r self,
        err_handler: &'r mut impl FnMut(SerializationError),
    ) -> SerializerCtx<'r, TransSerializerCtxState<'r>> {
        SerializerCtx::new(self.to_ctx_state(), err_handler)
    }

}
