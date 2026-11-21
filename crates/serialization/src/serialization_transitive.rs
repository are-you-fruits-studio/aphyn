use aphyn_ffi::{FfiAny, FfiAnyMut, FfiAnyRef, FfiFnMutMut};

use crate::{
    ContextData, ContextDataKey, SerializationError, SerializedValue, SerializerCtx, SerializerCtxState, TransSerializerFfi,
    TransSerializerFfiTypedRef, TransSerializerRegistry,
};

// todo: ffi

#[repr(C)]
#[derive(Copy, Clone)]
pub struct TransSerializerCtxState<'a> {
    registry: &'a TransSerializerRegistry,
    context: &'a ContextData<'a>,
}

impl<'a> TransSerializerCtxState<'a> {
    pub fn new(registry: &'a TransSerializerRegistry, context: &'a ContextData<'a>) -> Self {
        Self { registry, context }
    }

    pub fn registry(&self) -> &'a TransSerializerRegistry {
        self.registry
    }

    pub fn context(&self) -> &'a ContextData<'a> {
        self.context
    }

    pub fn into_ctx<'h>(self, err_handler: impl Into<FfiFnMutMut<'h, SerializationError, ()>>) -> SerializerCtx<'h, Self> {
        SerializerCtx::new(self, err_handler)
    }

    fn find_serializer<T: 'static>(&self) -> Option<TransSerializerFfiTypedRef<'a, T>> {
        self.registry.get::<T>()
    }

    fn find_serializer_virtual(&self, id: &str) -> Option<&'a TransSerializerFfi> {
        self.registry.get_virtual(id)
    }
}
impl<'a, T: 'static> SerializerCtxState<T> for TransSerializerCtxState<'a> {
    fn serialize(self, mut err_handler: FfiFnMutMut<SerializationError, ()>, value: &T, path: &str) -> SerializedValue {
        if let Some(serializer) = self.find_serializer::<T>() {
            let ctx = SerializerCtx::new(self, err_handler);
            return serializer.serialize(value, ctx, path);
        }

        err_handler.execute(SerializationError::MissingSerializer {
            type_name: std::any::type_name::<T>().into(),
        });
        SerializedValue::Null
    }

    fn deserialize(
        self,
        mut err_handler: FfiFnMutMut<SerializationError, ()>,
        value: &mut T,
        path: &str,
        serialized: &SerializedValue,
    ) {
        if let Some(serializer) = self.find_serializer::<T>() {
            let ctx = SerializerCtx::new(self, err_handler);
            serializer.deserialize(value, ctx, path, serialized);
            return;
        }

        err_handler.execute(SerializationError::MissingSerializer {
            type_name: std::any::type_name::<T>().into(),
        });
    }
}

impl<'a, 'b> SerializerCtx<'a, TransSerializerCtxState<'b>> {
    pub fn context_data<K: ContextDataKey>(&self) -> Option<&'b K::Value<'b>> {
        self.state().context.get::<K>()
    }

    pub fn serialize_any(&mut self, value: FfiAnyRef, path: &str) -> SerializedValue {
        let type_name = value.type_info().short().name();

        unsafe {
            if let Some(serializer) = self.state().find_serializer_virtual(type_name) {
                let ctx = SerializerCtx::new(self.state(), self.err_handler());
                return serializer.serialize_any(value, ctx, path);
            }
        }

        self.report_err(SerializationError::MissingSerializer {
            type_name: type_name.into(),
        });
        SerializedValue::Null
    }
    pub fn deserialize_any(&mut self, value: FfiAnyMut, path: &str, serialized: &SerializedValue) {
        unsafe {
            if let Some(serializer) = self.state().find_serializer_virtual(value.type_info().short().name()) {
                let ctx = SerializerCtx::new(self.state(), self.err_handler());
                serializer.deserialize_any(value, ctx, path, serialized);
                return;
            }
        }

        self.report_err(SerializationError::MissingSerializer {
            type_name: value.type_info().short().name().to_string().into(),
        });
    }

    pub fn deserialize_default_any(&mut self, id: &str, path: &str, serialized: &SerializedValue) -> Option<FfiAny> {
        unsafe {
            if let Some(serializer) = self.state().find_serializer_virtual(id) {
                let ctx = SerializerCtx::new(self.state(), self.err_handler());
                let mut value = serializer.serializable_default_any();
                serializer.deserialize_any(value.as_any_mut(), ctx, path, serialized);
                return Some(value);
            }
        }

        self.report_err(SerializationError::MissingSerializer {
            type_name: id.to_string().into(),
        });
        None
    }
}
