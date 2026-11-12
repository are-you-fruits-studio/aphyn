use aphyn_serialization::*;

#[repr(transparent)]
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Default)]
pub struct CollisionLayer(u8);

impl CollisionLayer {
    pub const MAX: u8 = 63;
    pub const COUNT: usize = Self::MAX as usize + 1;
    pub const DEFAULT: Self = Self(0);

    pub const fn new(layer: u8) -> Option<Self> {
        if layer > Self::MAX { None } else { Some(Self(layer)) }
    }

    pub const fn get(self) -> u8 {
        self.0
    }

    pub const fn mask_bit(self) -> u64 {
        1 << self.0
    }

    pub const fn is_in_mask(self, mask: u64) -> bool {
        mask & self.mask_bit() != 0
    }
}

pub const COLLISION_MASK_ALL: u64 = u64::MAX;

impl<S: Copy> Serializable<S> for CollisionLayer {
    fn serialize(&self, ctx: SerializerCtx<S>, path: &str) -> SerializedValue {
        Serializable::<S>::serialize(&self.0, ctx, path)
    }

    fn deserialize(&mut self, ctx: SerializerCtx<S>, path: &str, serialized: &SerializedValue) {
        let mut layer = self.0;
        Serializable::<S>::deserialize(&mut layer, ctx, path, serialized);
        self.0 = layer.min(Self::MAX);
    }
}
