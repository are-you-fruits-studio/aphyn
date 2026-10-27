use fruits_engine::*;

pub fn register_feature(mut world: WorldBuilderMut) {
    let mut data = world.data_mut();
    let mut res = data.resources_mut();
    let serializer = res.get_mut::<SerializersResource>().unwrap();

    {
        let serializer = &mut **serializer;

        register_self_and_related_common_transserializers::<CoordinateSpaceType>(serializer);
        // todo
    }
}
