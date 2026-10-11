use std::path::Path;

use fruits_asset_storage::AssetStorageResource;
use fruits_ecs::{ResourcesHolderMut, ResourcesHolderRef};
use fruits_ffi::FfiFnMutMut;
use fruits_render_core::{FilterMode, RenderApiResource, StandardTexture, StandardTextureAssetMetadata};

use fruits_serialization::{PureSerializerCtxState, SerializedValue, SerializerCtx, SerializerCtxState};
use image::EncodableLayout;

use crate::AssetLoader;

pub struct TextureHandleLoader<'a> {
    pub render_api: &'a RenderApiResource,
    pub textures: &'a mut AssetStorageResource<StandardTexture>,
}

impl<'a> TextureHandleLoader<'a> {
    pub fn from_world(res: ResourcesHolderMut<'a>) -> Option<Self> {
        Some(unsafe { Self {
            render_api: &*res.get_ptr::<RenderApiResource>()?,
            textures: &mut *res.get_ptr::<AssetStorageResource<StandardTexture>>()?,
        }})
    }
}

impl<'a, S: Copy + SerializerCtxState<StandardTextureAssetMetadata>> AssetLoader<S> for TextureHandleLoader<'a> {
    type Asset = StandardTexture;
    type SelfWithAnotherLifetime<'r> = TextureHandleLoader<'r>;

    fn create_loader<'r>(res: ResourcesHolderMut<'r>) -> Option<Self::SelfWithAnotherLifetime<'r>> {
        Self::SelfWithAnotherLifetime::from_world(res)
    }

    fn get_related_asset_storage(&mut self) -> &mut AssetStorageResource<Self::Asset> {
        self.textures
    }
    
    fn load_from_serialized(&mut self, mut ctx: SerializerCtx<S>, value: &SerializedValue, assets_dir_path: impl AsRef<Path>) -> Option<Self::Asset> {
        TextureLoader {
            render_api: self.render_api,
        }.load_from_deserialized(ctx.deserialize_default("", value), assets_dir_path)
    }
}

//

pub struct TextureLoader<'a> {
    pub render_api: &'a RenderApiResource,
}

impl<'a> TextureLoader<'a> {
    pub fn from_world(res: ResourcesHolderRef<'a>) -> Option<Self> {
        Some(Self {
            render_api: res.get::<RenderApiResource>()?,
        })
    }

    pub fn load_from_serialized(&mut self, value: &SerializedValue, assets_dir_path: impl AsRef<Path>) -> Option<StandardTexture> {
        let mut err_handler = |err| println!("[{}:{}] {err}", file!(), line!());
        let mut ctx = SerializerCtx::new(PureSerializerCtxState, &mut err_handler);
        let deserialized = ctx.deserialize_default::<StandardTextureAssetMetadata>("", value);

        self.load_from_deserialized(
            deserialized,
            assets_dir_path,
        )
    }

    pub fn load_from_deserialized(&mut self, deserialized: StandardTextureAssetMetadata, assets_dir_path: impl AsRef<Path>) -> Option<StandardTexture> {

        let mut path = assets_dir_path.as_ref().to_path_buf();

        path.push(deserialized.raw_texture.as_str());

        let texture_bytes = std::fs::read(path).ok()?;

        self.load_from_bytes(&texture_bytes, deserialized)
    }

    pub fn load_from_bytes(&mut self, bytes: &[u8], meta: StandardTextureAssetMetadata) -> Option<StandardTexture> {
        let img = image::load_from_memory(bytes).ok()?.to_rgba8();

        let texture = self.render_api.create_texture(FilterMode::Nearest, img.dimensions().into(), &img.as_bytes(), meta);

        Some(texture)
    }
}
