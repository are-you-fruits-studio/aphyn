use fruits_ffi::{FfiFnMutMut, FfiString, FfiVec};

use crate::{
    CoreEnumDeserializerCtx, CoreListDeserializerCtx, CoreListSerializerCtx, CoreMapDeserializerCtx, CoreMapSerializerCtx, SerializationError, SerializedValue, decompose_serialization_path, deserialize_list_inverted, deserialize_map_inverted,
};

// todo: ffi

pub trait Serializable<S: Copy>: Sized {
    fn serialize(&self, ctx: SerializerCtx<S>, path: &str) -> SerializedValue;
    fn deserialize(&mut self, ctx: SerializerCtx<S>, path: &str, serialized: &SerializedValue);
}

pub trait SerializerCtxState<T>: Copy {
    fn serialize(self, err_handler: FfiFnMutMut<SerializationError, ()>, value: &T, path: &str) -> SerializedValue;
    fn deserialize(
        self,
        err_handler: FfiFnMutMut<SerializationError, ()>,
        value: &mut T,
        path: &str,
        serialized: &SerializedValue,
    );
}

#[repr(C)]
pub struct SerializerCtx<'a, S: Copy> {
    state: S,
    err_handler: FfiFnMutMut<'a, SerializationError, ()>,
}

impl<'a, S: Copy> SerializerCtx<'a, S> {
    pub fn new(state: S, err_handler: impl Into<FfiFnMutMut<'a, SerializationError, ()>>) -> Self {
        Self {
            state,
            err_handler: err_handler.into(),
        }
    }

    pub fn as_mut<'r>(&'r mut self) -> SerializerCtx<'r, S>
    where
        'a: 'r,
    {
        SerializerCtx {
            state: self.state,
            err_handler: self.err_handler.as_mut(),
        }
    }

    pub(crate) fn state(&self) -> S {
        self.state
    }

    pub(crate) fn err_handler<'r>(&'r mut self) -> FfiFnMutMut<'r, SerializationError, ()>
        where 'a: 'r
    {
        self.err_handler.as_mut()
    }

    pub fn map_state<U: Copy>(self, f: impl FnOnce(S) -> U) -> SerializerCtx<'a, U> {
        SerializerCtx {
            state: f(self.state),
            err_handler: self.err_handler,
        }
    }

    pub fn map_state_mut<'r, U: Copy>(&'r mut self, f: impl FnOnce(&'r mut S) -> U) -> SerializerCtx<'r, U>
        where 'a: 'r
    {
        SerializerCtx {
            state: f(&mut self.state),
            err_handler: self.err_handler.as_mut(),
        }
    }

    pub fn report_err(&mut self, err: SerializationError) {
        self.err_handler.execute(err);
    }

    pub fn deserialize_map<'r, T: Default>(
        &'r mut self,
        value: &mut T,
        path: &'r str,
        serialized: &'r SerializedValue,
    ) -> MapDeserializerCtx<'r, S>
    where
        'a: 'r,
    {
        self.deserialize_map_custom_default(value, path, serialized, || Default::default())
    }

    pub fn deserialize_map_custom_default<'r, T>(
        &'r mut self,
        value: &mut T,
        path: &'r str,
        serialized: &'r SerializedValue,
        fn_default: impl FnOnce() -> T,
    ) -> MapDeserializerCtx<'r, S>
    where
        'a: 'r,
    {
        let path = decompose_serialization_path(path);

        if path.is_none() {
            *value = fn_default();
        }

        MapDeserializerCtx::new(self.as_mut(), path, serialized)
    }

    pub fn deserialize_map_fields<'r>(&'r mut self, path: &'r str, serialized: &'r SerializedValue) -> MapDeserializerCtx<'r, S>
    where
        'a: 'r,
    {
        let path = decompose_serialization_path(path);

        MapDeserializerCtx::new(self.as_mut(), path, serialized)
    }

    pub fn deserialize_map_inverted<T: Default>(
        mut self,
        value: &mut T,
        path: &str,
        serialized: &SerializedValue,
        mut fields_handler: impl FnMut(&mut T, SerializerCtx<S>, &str, &str, &SerializedValue),
    ) {
        let path = decompose_serialization_path(path);

        if path.is_none() {
            *value = Default::default();
        }

        deserialize_map_inverted(path, serialized, |field_name, path, serialized| {
            fields_handler(value, self.as_mut(), field_name, path, serialized)
        });
    }

    pub fn deserialize_list<'r, T: Default>(
        &'r mut self,
        value: &mut T,
        path: &'r str,
        serialized: &'r SerializedValue,
    ) -> ListDeserializerCtx<'r, S>
    where
        'a: 'r,
    {
        self.deserialize_list_custom_default(value, path, serialized, || Default::default())
    }

    pub fn deserialize_list_inverted<T: Default>(
        mut self,
        value: &mut T,
        path: &str,
        serialized: &SerializedValue,
        mut elements_handler: impl FnMut(&mut T, SerializerCtx<S>, usize, &str, &SerializedValue),
    ) {
        let path = decompose_serialization_path(path);

        if path.is_none() {
            *value = Default::default();
        }

        deserialize_list_inverted(path, serialized, |idx, path, serialized| {
            elements_handler(value, self.as_mut(), idx, path, serialized)
        });
    }

    pub fn deserialize_list_custom_default<'r, T>(
        &'r mut self,
        value: &mut T,
        path: &'r str,
        serialized: &'r SerializedValue,
        fn_default: impl FnOnce() -> T,
    ) -> ListDeserializerCtx<'r, S>
    where
        'a: 'r,
    {
        let path = decompose_serialization_path(path);

        if path.is_none() {
            *value = fn_default();
        }

        ListDeserializerCtx::new(self.as_mut(), path, serialized)
    }

    pub fn deserialize_list_elements<'r, T>(
        &'r mut self,
        path: &'r str,
        serialized: &'r SerializedValue,
    ) -> ListDeserializerCtx<'r, S>
    where
        'a: 'r,
    {
        let path = decompose_serialization_path(path);

        ListDeserializerCtx::new(self.as_mut(), path, serialized)
    }

    pub fn deserialize_enum<'r, T>(&'r mut self, path: &'r str, serialized: &'r SerializedValue) -> EnumDeserializerCtx<'r, S, T>
    where
        'a: 'r,
    {
        EnumDeserializerCtx::new(self.as_mut(), path, serialized)
    }

    pub fn serialize_map<'r>(&'r mut self, path: &'r str) -> MapSerializerCtx<'r, S>
    where
        'a: 'r,
    {
        MapSerializerCtx::new(self.as_mut(), decompose_serialization_path(path))
    }

    pub fn serialize_list<'r>(&'r mut self, path: &'r str) -> ListSerializerCtx<'r, S>
    where
        'a: 'r,
    {
        ListSerializerCtx::new(self.as_mut(), decompose_serialization_path(path))
    }

    pub fn deserialize_default<T: Default>(&mut self, path: &str, serialized: &SerializedValue) -> T
    where
        S: SerializerCtxState<T>,
    {
        self.deserialize_custom_default(path, serialized, Default::default())
    }

    pub fn deserialize_custom_default<T>(&mut self, path: &str, serialized: &SerializedValue, mut default: T) -> T
    where
        S: SerializerCtxState<T>,
    {
        self.deserialize(&mut default, path, serialized);
        default
    }

    pub fn serialize<T>(&mut self, value: &T, path: &str) -> SerializedValue
    where
        S: SerializerCtxState<T>,
    {
        self.state.serialize(self.err_handler.as_mut(), value, path)
    }

    pub fn deserialize<T>(&mut self, value: &mut T, path: &str, serialized: &SerializedValue)
    where
        S: SerializerCtxState<T>,
    {
        self.state.deserialize(self.err_handler.as_mut(), value, path, serialized)
    }
}

//

pub struct MapSerializerCtx<'a, S: Copy> {
    ctx: SerializerCtx<'a, S>,
    core: CoreMapSerializerCtx<'a>,
}

impl<'a, S: Copy> MapSerializerCtx<'a, S> {
    fn new(ctx: SerializerCtx<'a, S>, path: Option<(&'a str, &'a str)>) -> Self {
        Self {
            ctx,
            core: CoreMapSerializerCtx::new(path),
        }
    }

    pub fn with_field<T>(mut self, name: &str, value: &T) -> Self
    where
        S: SerializerCtxState<T>,
    {
        self.core.add_field(name, |path| self.ctx.serialize(value, path));
        self
    }

    pub fn finish_as_map(self, is_rigid: bool) -> SerializedValue {
        self.core.finish_as_map(is_rigid)
    }

    pub fn finish_as_enum(self, is_rigid: bool, variant: impl Into<FfiString>, variants: FfiVec<FfiString>) -> SerializedValue {
        self.core.finish_as_enum(is_rigid, variant, variants)
    }
}

// todo: ffi
pub struct ListSerializerCtx<'a, S: Copy> {
    ctx: SerializerCtx<'a, S>,
    core: CoreListSerializerCtx<'a>,
}

impl<'a, S: Copy> ListSerializerCtx<'a, S> {
    pub fn new(ctx: SerializerCtx<'a, S>, path: Option<(&'a str, &'a str)>) -> Self {
        Self {
            ctx,
            core: CoreListSerializerCtx::new(path),
        }
    }

    pub fn with_element<T>(mut self, idx: usize, value: &T) -> Self
    where
        S: SerializerCtxState<T>,
    {
        self.core.add_element(idx, |path| self.ctx.serialize(value, path));
        self
    }

    pub fn finish_as_list(self, is_rigid: bool) -> SerializedValue {
        self.core.finish_as_list(is_rigid)
    }
}

//

// todo: ffi
pub struct MapDeserializerCtx<'a, S: Copy> {
    ctx: SerializerCtx<'a, S>,
    core: CoreMapDeserializerCtx<'a>,
}

impl<'a, S: Copy> MapDeserializerCtx<'a, S> {
    fn new(ctx: SerializerCtx<'a, S>, path: Option<(&'a str, &'a str)>, serialized: &'a SerializedValue) -> Self {
        Self {
            ctx,
            core: CoreMapDeserializerCtx::new(path, serialized),
        }
    }

    pub fn with_field<T>(mut self, name: &str, value: &mut T) -> Self
    where
        S: SerializerCtxState<T>,
    {
        self.core
            .add_field(name, |path, serialized| self.ctx.deserialize(value, path, serialized));
        self
    }
}

//

// todo: ffi
pub struct ListDeserializerCtx<'a, S: Copy> {
    ctx: SerializerCtx<'a, S>,
    core: CoreListDeserializerCtx<'a>,
}

impl<'a, S: Copy> ListDeserializerCtx<'a, S> {
    fn new(ctx: SerializerCtx<'a, S>, path: Option<(&'a str, &'a str)>, serialized: &'a SerializedValue) -> Self {
        Self {
            ctx,
            core: CoreListDeserializerCtx::new(path, serialized),
        }
    }

    pub fn with_element<T>(mut self, idx: usize, value: &mut T) -> Self
    where
        S: SerializerCtxState<T>,
    {
        self.core
            .add_element(idx, |path, serialized| self.ctx.deserialize(value, path, serialized));
        self
    }
}

// todo: ffi
pub struct EnumDeserializerCtx<'a, S: Copy, T> {
    ctx: SerializerCtx<'a, S>,
    core: CoreEnumDeserializerCtx<'a, T>,
}

impl<'a, S: Copy, T> EnumDeserializerCtx<'a, S, T> {
    pub fn new(ctx: SerializerCtx<'a, S>, path: &'a str, serialized: &'a SerializedValue) -> Self {
        Self {
            ctx,
            core: CoreEnumDeserializerCtx::new(path, serialized),
        }
    }

    pub fn with_variant(mut self, variant: impl Into<FfiString>, default: impl 'a + FnOnce() -> T) -> Self {
        self.core.add_variant(variant, default);
        self
    }

    pub fn finish(mut self, value: &mut T, deserializer: impl for<'b> FnOnce(&'b mut T, MapDeserializerCtx<'b, S>)) {
        self.core.finish(
            value,
            &mut self.ctx,
            |ctx, value, path, serialized| deserializer(value, ctx.deserialize_map_fields(path, serialized)),
            |state, err| state.report_err(err),
        )
    }
}
