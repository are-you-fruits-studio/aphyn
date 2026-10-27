use std::ops::Deref;

use crate::*;

pub struct Evt<'e, E: Event> {
    evt: &'e [E],
}

impl<'e, E: Event> Deref for Evt<'e, E> {
    type Target = [E];

    fn deref(&self) -> &Self::Target {
        self.evt
    }
}

unsafe impl<'e, E: Event> SystemParam for Evt<'e, E> {
    type Item<'b> = Evt<'b, E>;

    fn fill_data_usage(usage: &mut DataUsageBuilder, types: &TypesRegistryCache) {
        usage.world().events().add_type(DataUsageEntry {
            type_id: types.get_or_register::<E>(),
            details: DataUsageDetails {
                is_mut: false,
                is_required: true,
            },
        });
    }

    unsafe fn new<'a>(input: &'a SystemInput<'a>) -> Result<Self::Item<'a>, &'static str> {
        Ok(Evt {
            // Safety. Managed by caller.
            evt: input.world_data.events().get::<E>(),
        })
    }
}
