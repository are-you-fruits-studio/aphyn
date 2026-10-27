use aphyn::*;

#[derive(Resource, Clone, Default)]
pub struct UiInteractionResource {
    pub selection: EntityId,
    pub drag: EntityId,
}

#[derive(Resource, Clone)]
pub struct StandardAssetsResource {
    pub material_panel: AssetHandle<StandardMaterial>,
    pub material_text: AssetHandle<StandardMaterial>,
    pub font: AssetHandle<Font>,
}
