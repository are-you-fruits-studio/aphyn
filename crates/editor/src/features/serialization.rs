use aphyn::*;

// todo: bundle registration params
pub fn register_feature(mut data: WorldDataMut, _behavior: WorldBehaviorBuilderMut, _render: &mut RenderFeatureRegistry) {
    let mut data = data.as_mut();
    let mut res = data.resources_mut();
    let serializer = res.get_mut::<SerializersResource>().unwrap();

    {
        let serializer = &mut **serializer;

        register_self_and_related_common_transserializers::<CoordinateSpaceType>(serializer);
        // todo
    }
}
