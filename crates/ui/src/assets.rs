use std::collections::HashMap;
use aphyn_asset_storage::AssetHandle;
use aphyn_math::Vec2;
use aphyn_render_core::StandardTexture;

// todo: ffi?
pub struct Font {
    pub texture: AssetHandle<StandardTexture>,
    pub characters_uv: HashMap<char, [Vec2<f32>; 2]>,
    pub missing_character_uv: [Vec2<f32>; 2],
    pub character_ratio: f32,
}
