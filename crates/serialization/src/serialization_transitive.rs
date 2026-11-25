use aphyn_any::{AnyBox, AnyMut, AnyRef, AnyTypeId, TypesRegistryCache};
use aphyn_ffi::FfiFnMutMut;

use crate::{
    ContextData, ContextDataKey, SerializationError, SerializedValue, SerializerCtx, SerializerCtxState, TransSerializerEntry,
    TransSerializerFfiTypedRef, TransSerializerRegistry,
};

// todo: ffi

#[repr(C)]
#[derive(Copy, Clone)]
pub struct TransSerializerCtxState<'a> {
    registry: &'a TransSerializerRegistry,
    context: &'a ContextData<'a>,
    // always a cache of the binary that runs the code holding this state: each serializer trampoline swaps in
    // the cache of the binary that registered the serializer
    local_types: &'a TypesRegistryCache,
}

impl<'a> TransSerializerCtxState<'a> {
    pub fn new(registry: &'a TransSerializerRegistry, context: &'a ContextData<'a>, local_types: &'a TypesRegistryCache) -> Self {
        Self {
            registry,
            context,
            local_types,
        }
    }

    pub fn registry(&self) -> &'a TransSerializerRegistry {
        self.registry
    }

    pub fn context(&self) -> &'a ContextData<'a> {
        self.context
    }

    pub fn local_types(&self) -> &'a TypesRegistryCache {
        self.local_types
    }

    pub(crate) fn with_local_types(self, local_types: &'a TypesRegistryCache) -> Self {
        Self { local_types, ..self }
    }

    pub fn into_ctx<'h>(self, err_handler: impl Into<FfiFnMutMut<'h, SerializationError, ()>>) -> SerializerCtx<'h, Self> {
        SerializerCtx::new(self, err_handler)
    }

    fn find_serializer<T: 'static>(&self) -> Option<TransSerializerFfiTypedRef<'a, T>> {
        self.registry.get::<T>(self.local_types)
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
    pub fn registry(&self) -> &'b TransSerializerRegistry {
        self.state().registry
    }

    pub fn local_types(&self) -> &'b TypesRegistryCache {
        self.state().local_types
    }

    pub fn context_data<K: ContextDataKey>(&self) -> Option<&'b K::Value<'b>> {
        self.state().context.get::<K>()
    }

    pub fn serialize_any(&mut self, value: AnyRef, path: &str) -> SerializedValue {
        let state = self.state();

        if let Some(entry) = state.registry.get_for(value.type_id(), value.type_info()) {
            let ctx = SerializerCtx::new(state, self.err_handler());
            return unsafe { entry.serializer().serialize_any(value, ctx, path) };
        }

        self.report_err(SerializationError::MissingSerializer {
            type_name: value.type_info().short().name().into(),
        });
        SerializedValue::Null
    }

    pub fn deserialize_any(&mut self, value: AnyMut, path: &str, serialized: &SerializedValue) {
        let state = self.state();

        if let Some(entry) = state.registry.get_for(value.type_id(), value.type_info()) {
            let ctx = SerializerCtx::new(state, self.err_handler());
            unsafe { entry.serializer().deserialize_any(value, ctx, path, serialized) };
            return;
        }

        self.report_err(SerializationError::MissingSerializer {
            type_name: value.type_info().short().name().into(),
        });
    }

    // `name` is the serialized name the type was registered with
    pub fn deserialize_default_any(&mut self, name: &str, path: &str, serialized: &SerializedValue) -> Option<AnyBox> {
        let state = self.state();

        match state.registry.get_by_name(name) {
            Some(entry) => Some(self.deserialize_default_entry(entry, path, serialized)),
            None => {
                self.report_err(SerializationError::MissingSerializer { type_name: name.into() });
                None
            }
        }
    }

    pub fn deserialize_default_any_by_id(&mut self, id: AnyTypeId, path: &str, serialized: &SerializedValue) -> Option<AnyBox> {
        let state = self.state();

        match state.registry.get_by_id(id) {
            Some(entry) => Some(self.deserialize_default_entry(entry, path, serialized)),
            None => {
                self.report_err(SerializationError::MissingSerializer {
                    type_name: format!("type #{}", id.id()).into(),
                });
                None
            }
        }
    }

    fn deserialize_default_entry(&mut self, entry: &'b TransSerializerEntry, path: &str, serialized: &SerializedValue) -> AnyBox {
        let state = self.state();
        let serializer = entry.serializer();

        unsafe {
            let ctx = SerializerCtx::new(state, self.err_handler());
            let mut value = serializer.serializable_default_any(state.registry.types());
            serializer.deserialize_any(value.as_any_mut(), ctx, path, serialized);
            value
        }
    }
}
