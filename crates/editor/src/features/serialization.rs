use aphyn::*;

pub fn register_feature(ctx: FeatureInitCtx) {
    register_self_and_related_common_transserializers::<CoordinateSpaceType>(
        ctx.types,
        "aphyn::CoordinateSpaceType",
        ctx.serializers,
    );
    // todo
}
