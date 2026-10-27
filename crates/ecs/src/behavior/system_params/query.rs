use crate::*;

pub type WorldQuery<'e, A, F = ()> = EntitiesHolderQuery<'e, A, F>;

// todo: unsafe to sealed trait
unsafe impl<'e, A: ArchetypeIteratorItem, F: QueryFilter> SystemParam for WorldQuery<'e, A, F> {
    type Item<'b> = WorldQuery<'b, A::Item<'b>, F>;

    fn fill_data_usage(usage: &mut DataUsageBuilder, types: &TypesRegistryCache) {
        let usage = usage.world().entities().as_by_type();

        usage.add_type(DataUsageEntry {
            type_id: types.get_or_register::<EntityId>(),
            details: DataUsageDetails {
                is_mut: false,
                is_required: true,
            },
        });

        A::fill_usage(usage, types);
    }

    unsafe fn new<'a>(input: &'a SystemInput<'a>) -> Result<Self::Item<'a>, &'static str> {
        // Safety. Managed by caller.
        Ok(unsafe { input.world_data.entities().unsafe_query::<A::Item<'_>, F>() })
    }
}
