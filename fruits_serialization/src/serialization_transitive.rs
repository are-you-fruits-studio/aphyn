use fruits_ffi::{FfiAny, FfiAnyMut, FfiAnyRef, FfiFnMutMut, FfiOption};

use crate::{SerializationError, SerializedValue, SerializerCtx, SerializerCtxState, TransSerializerFfi, TransSerializerFfiTypedRef, TransSerializerRegistry};

// todo: ffi

// todo: ctx wraps wider ctx for scoping
// mod sealed {
//     pub trait Sealed {}
// }
// pub trait SerializerCtxTrait : sealed::Sealed {
//     type WiderCtx;
// }
// impl<'a, 'l: 'a> sealed::Sealed for SerializerCtx<'a, 'l> {
// }

#[repr(C)]
#[derive(Copy, Clone)]
pub struct TransSerializerCtxState<'a> {
    registry: &'a TransSerializerRegistry<'a>,
    wider_ctx: FfiOption<&'a TransSerializerCtxState<'a>>,
}

impl<'a> TransSerializerCtxState<'a> {
    pub fn new(
        registry: &'a TransSerializerRegistry<'a>,
        wider_ctx: Option<&'a TransSerializerCtxState<'a>>,
    ) -> Self {
        Self {
            registry,
            wider_ctx: wider_ctx.into(),
        }
    }

    fn find_serializer<'r, T: 'static>(&'r self) -> Option<TransSerializerFfiTypedRef<'r, 'r, T>>
        where 'a: 'r
    {
        // todo: recursion to loop

        if let Some(serializer) = self.registry.get::<T>() {
            return Some(serializer);
        }

        if let Some(wider_ctx) = self.wider_ctx.as_option() {
            return wider_ctx.find_serializer()
        }

        None
    }

    fn find_serializer_virtual<'r>(&'r self, id: &str) -> Option<&'r TransSerializerFfi<'r>>
        where 'a: 'r
    {
        // todo: recursion to loop

        if let Some(serializer) = self.registry.get_virtual(id) {
            return Some(serializer);
        }

        if let Some(wider_ctx) = self.wider_ctx.as_option() {
            return wider_ctx.find_serializer_virtual(id)
        }

        None
    }

    pub fn wrap_with_local(&'a self, local: &'a TransSerializerRegistry<'a>) -> TransSerializerCtxState<'a> {
        TransSerializerCtxState::new(local, Some(self))
    }

    pub fn into_ctx(self, err_handler: impl Into<FfiFnMutMut<'a, SerializationError, ()>>) -> SerializerCtx<'a, Self> {
        SerializerCtx::new(self, err_handler)
    }
}
impl<'a, T: 'static> SerializerCtxState<T> for TransSerializerCtxState<'a> {
    fn serialize(self, mut err_handler: FfiFnMutMut<SerializationError, ()>, value: &T, path: &str) -> SerializedValue {
        if let Some(serializer) = self.find_serializer::<T>() {
            let ctx = SerializerCtx::new(self, err_handler);
            return serializer.serialize(value, ctx, path);
        }

        err_handler.execute(SerializationError::MissingSerializer { type_name: std::any::type_name::<T>().into() });
        SerializedValue::Null
    }

    fn deserialize(self, mut err_handler: FfiFnMutMut<SerializationError, ()>, value: &mut T, path: &str, serialized: &SerializedValue) {
        if let Some(serializer) = self.find_serializer::<T>() {
            let ctx = SerializerCtx::new(self, err_handler);
            serializer.deserialize(value, ctx, path, serialized);
            return;
        }

        err_handler.execute(SerializationError::MissingSerializer { type_name: std::any::type_name::<T>().into() });
    }
}

impl<'a, 'b> SerializerCtx<'a, TransSerializerCtxState<'b>> {
    pub fn serialize_any(&mut self, value: FfiAnyRef, path: &str) -> SerializedValue {
        let type_name = value.type_info().short().name();

        unsafe {
            if let Some(serializer) = self.state().find_serializer_virtual(type_name) {
                let ctx = SerializerCtx::new(self.state(), self.err_handler());
                return serializer.serialize_any(value, ctx, path);
            }
        }

        self.report_err(SerializationError::MissingSerializer { type_name: type_name.into() });
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

        self.report_err(SerializationError::MissingSerializer { type_name: value.type_info().short().name().to_string().into() });
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

        self.report_err(SerializationError::MissingSerializer { type_name: id.to_string().into() });
        None
    }

}

// todo: support FfiAny (again)
// impl<'a, 'l: 'a> TransSerializerCtxState<'a, 'l> {
//     pub fn serialize_any(&mut self, value: FfiAnyRef) -> SerializedValue {
//         let type_name = value.type_info().short().name();
       
//         unsafe {
//             if let Some(serializer) = self.registry_local.as_option().map(|r| r.get_virtual(type_name)).flatten() {
//             let ctx = SerializerCtx::new(self, err_handler);
//                 return serializer.serialize_any(ctx, value);
//             }
//             if let Some(serializer) = self.registry_global.get_virtual(type_name) {
//                 return serializer.serialize_any(self.as_mut(), value);
//             }
//         }

//         self.report_err(SerializationError::MissingSerializer { type_name: type_name.into() });
//         SerializedValue::Null
//     }
//     pub fn deserialize_any(&mut self, id: &str, data: &SerializedValue) -> Option<FfiAny> {
//         if let Some(serializer) = self.registry_local.as_option().map(|r| r.get_virtual(id)).flatten() {
//             let ctx = SerializerCtx::new(self, err_handler);
//             return serializer.deserialize_any(ctx, data);
//         }
//         if let Some(serializer) = self.registry_global.get_virtual(id) {
//             return serializer.deserialize_any(self.as_mut(), data);
//         }

//         self.report_err(SerializationError::MissingSerializer { type_name: id.to_string().into() });
//         None
//     }
// }
