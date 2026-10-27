use std::ops::{BitAnd, BitOr};

use fruits_serialization::*;

// todo: rewrite Debug impl
#[repr(transparent)]
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub struct LineBoundType(u8);
impl Default for LineBoundType {
    fn default() -> Self {
        Self::SEGMENT
    }
}
impl LineBoundType {
    pub const UNRESTRICTED: LineBoundType = LineBoundType(0);
    pub const START_RESTRICTED: LineBoundType = LineBoundType(1 << 0);
    pub const END_RESTRICTED: LineBoundType = LineBoundType(1 << 1);
    pub const SEGMENT: LineBoundType = LineBoundType(LineBoundType::START_RESTRICTED.0 | LineBoundType::END_RESTRICTED.0);

    pub const fn is_start_restricted(&self) -> bool {
        self.0 & Self::START_RESTRICTED.0 != 0
    }
    pub const fn is_end_restricted(&self) -> bool {
        self.0 & Self::END_RESTRICTED.0 != 0
    }
}
impl BitOr for LineBoundType {
    type Output = LineBoundType;

    fn bitor(self, rhs: Self) -> Self::Output {
        LineBoundType(self.0 | rhs.0)
    }
}
impl BitAnd for LineBoundType {
    type Output = LineBoundType;

    fn bitand(self, rhs: Self) -> Self::Output {
        LineBoundType(self.0 & rhs.0)
    }
}
impl<S: Copy> Serializable<S> for LineBoundType {
    fn serialize(&self, mut ctx: SerializerCtx<S>, path: &str) -> SerializedValue {
        let variants = ["UNRESTRICTED", "START_RESTRICTED", "END_RESTRICTED", "SEGMENT"].into_iter().map(|s| s.into()).collect();

        let stringified = match self {
            &Self::UNRESTRICTED => "UNRESTRICTED",
            &Self::START_RESTRICTED => "START_RESTRICTED",
            &Self::END_RESTRICTED => "END_RESTRICTED",
            &Self::SEGMENT => "SEGMENT",
            _ => "INVALID",
        };

        ctx.serialize_map(path)
            .finish_as_enum(true, stringified, variants)
    }

    fn deserialize(&mut self, mut ctx: SerializerCtx<S>, path: &str, serialized: &SerializedValue) {
        ctx.deserialize_enum(path, serialized)
            .with_variant("UNRESTRICTED", || Self::UNRESTRICTED)
            .with_variant("START_RESTRICTED", || Self::START_RESTRICTED)
            .with_variant("END_RESTRICTED", || Self::END_RESTRICTED)
            .with_variant("SEGMENT", || Self::SEGMENT)
            .finish(self, |_, _| {});
    }
}