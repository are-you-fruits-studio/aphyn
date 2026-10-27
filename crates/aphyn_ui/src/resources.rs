use aphyn_asset_storage::AssetHandle;
use aphyn_ecs::Resource;
use aphyn_render_core::StandardTexture;

use crate::Font;

#[repr(C)]
#[derive(Resource)]
pub struct StandardUiAssetsResource {
    pub texture_white: AssetHandle<StandardTexture>,
    pub texture_text_px_5_7: AssetHandle<StandardTexture>,
    pub font_px_5_7: AssetHandle<Font>,
    pub texture_text_px_8_8: AssetHandle<StandardTexture>,
    pub font_px_8_8: AssetHandle<Font>,
    pub texture_text_px_8_12: AssetHandle<StandardTexture>,
    pub font_px_8_12: AssetHandle<Font>,
}
