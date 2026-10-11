use fruits_ffi::FfiFnMutMut ;

use crate::{
    Serializable, SerializationError, SerializedValue, SerializerCtxState, serialization_ctx::SerializerCtx,
};

// todo: ffi

#[derive(Copy, Clone)]
pub struct PureSerializerCtxState;

impl<T: Serializable<PureSerializerCtxState>> SerializerCtxState<T> for PureSerializerCtxState {
    fn serialize(self, err_handler: FfiFnMutMut<SerializationError, ()>, deserialized: &T, path: &str) -> SerializedValue {
        deserialized.serialize(SerializerCtx::new(self, err_handler), path)
    }

    fn deserialize(self, err_handler: FfiFnMutMut<SerializationError, ()>, deserialized: &mut T, path: &str, serialized: &SerializedValue) {
        deserialized.deserialize(SerializerCtx::new(self, err_handler), path, serialized);
    }
}
