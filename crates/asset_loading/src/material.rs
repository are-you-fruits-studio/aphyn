use aphyn_asset_storage::AssetStorageResource;
use aphyn_ecs::{ResourcesHolderMut, ResourcesHolderRef};
use aphyn_render::{RenderApiResource, StandardMaterial, StandardMaterialAssetMetadata};
use aphyn_serialization::{SerializedValue, SerializerCtx, SerializerCtxState};

pub struct MaterialHandleLoader<'a> {
    pub render_api: &'a RenderApiResource,
    pub materials: &'a mut AssetStorageResource<StandardMaterial>,
}

impl<'a> MaterialHandleLoader<'a> {
    pub fn from_world(res: ResourcesHolderMut<'a>) -> Option<Self> {
        Some(unsafe { Self {
            render_api: &*res.get_ptr::<RenderApiResource>()?,
            materials: &mut *res.get_ptr::<AssetStorageResource<StandardMaterial>>()?,
        }})
    }
}

//

pub struct MaterialLoader<'a> {
    pub render_api: &'a RenderApiResource,
}

impl<'a> MaterialLoader<'a> {
    pub fn from_world(res: ResourcesHolderRef<'a>) -> Option<Self> {
        Some(Self {
            render_api: res.get::<RenderApiResource>()?,
        })
    }

    pub fn load_from_serialized<S: Copy + SerializerCtxState<StandardMaterialAssetMetadata>>(
        &mut self,
        mut ctx: SerializerCtx<S>,
        value: &SerializedValue,
    ) -> Option<StandardMaterial> {
        let asset_metadata = ctx.deserialize_default::<StandardMaterialAssetMetadata>("", value);

        self.load_from_deserialized(asset_metadata)
    }

    pub fn load_from_deserialized(&mut self, asset_metadata: StandardMaterialAssetMetadata) -> Option<StandardMaterial> {
        Some(self.render_api.create_material(asset_metadata))
    }
}
