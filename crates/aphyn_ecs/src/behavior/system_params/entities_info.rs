use crate::*;

pub struct EntitiesInfo<'e> {
    ec: EntitiesHolderRef<'e>,
}

impl<'e> EntitiesInfo<'e> {
    pub fn exists(&self, entity: EntityId) -> bool {
        self.ec.contains_entity(entity)
    }

    pub fn entities_count(&self) -> u64 {
        self.ec.entities_count()
    }
}

unsafe impl<'e> SystemParam for EntitiesInfo<'e> {
    type Item<'b> = EntitiesInfo<'b>;

    fn fill_data_usage(usage: &mut DataUsageBuilder, types: &TypesRegistryCache) {
        usage.world().entities().add_type(DataUsageEntry {
            type_id: types.get_or_register::<EntityId>(),
            details: DataUsageDetails {
                is_mut: false,
                is_required: true,
            },
        });
    }

    unsafe fn new<'a>(input: &'a SystemInput<'a>) -> Result<Self::Item<'a>, &'static str> {
        Ok(EntitiesInfo {
            // Safety. Managed by caller
            ec: input.world_data.entities(),
        })
    }
}
