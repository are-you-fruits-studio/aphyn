use std::path::Path;

use aphyn_asset_storage::AssetStorageResource;
use aphyn_render::{RenderApiResource, StandardTexture};
use aphyn_ui::{Font, FontAssetMetadata, load_font};

pub struct FontLoader<'a> {
    pub render_api: &'a RenderApiResource,
}

impl<'a> FontLoader<'a> {
    pub fn load_from_deserialized(
        &mut self,
        meta: FontAssetMetadata,
        textures: &mut AssetStorageResource<StandardTexture>,
        assets_dir_path: impl AsRef<Path>,
    ) -> Option<Font> {
        let path = assets_dir_path.as_ref().join(meta.raw_font.as_str());

        let bytes = match std::fs::read(&path) {
            Ok(bytes) => bytes,
            Err(err) => {
                println!("failed to read font {path:?}. {err}");
                return None;
            }
        };

        match load_font(self.render_api, textures, &bytes, &meta.raster_settings()) {
            Ok(font) => Some(font),
            Err(err) => {
                println!("failed to load font {path:?}. {err}");
                None
            }
        }
    }
}
