use aphyn::*;

#[derive(Resource, Clone, Default)]
pub struct UiInteractionResource {
    pub selection: EntityId,
    pub drag: EntityId,
}

#[derive(Resource, Clone)]
pub struct StandardAssetsResource {
    pub material_panel: AssetHandle<Material>,
    pub material_text: AssetHandle<Material>,
    pub font: AssetHandle<Font>,
}
