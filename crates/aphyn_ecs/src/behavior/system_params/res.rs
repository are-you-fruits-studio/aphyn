use std::ops::Deref;

use crate::*;

pub struct Res<'e, R: Resource> {
    res: &'e R,
}

impl<'e, R: Resource> Deref for Res<'e, R> {
    type Target = R;

    fn deref(&self) -> &Self::Target {
        self.res
    }
}

unsafe impl<'e, R: Resource> SystemParam for Res<'e, R> {
    type Item<'b> = Res<'b, R>;

    fn fill_data_usage(usage: &mut DataUsageBuilder, types: &TypesRegistryCache) {
        usage.world().resources().add_type(DataUsageEntry {
            type_id: types.get_or_register::<R>(),
            details: DataUsageDetails {
                is_mut: false,
                is_required: true,
            },
        });
    }

    unsafe fn new<'a>(input: &'a SystemInput<'a>) -> Result<Self::Item<'a>, &'static str> {
        Ok(Res {
            // Safety. Managed by caller.
            res: unsafe { &*input.world_data.resources().get_ptr::<R>().ok_or("Resource missing.")? },
        })
    }
}

unsafe impl<'e, R: Resource> SystemParam for Option<Res<'e, R>> {
    type Item<'b> = Option<Res<'b, R>>;

    fn fill_data_usage(usage: &mut DataUsageBuilder, types: &TypesRegistryCache) {
        usage.world().resources().add_type(DataUsageEntry {
            type_id: types.get_or_register::<R>(),
            details: DataUsageDetails {
                is_mut: false,
                is_required: false,
            },
        });
    }

    unsafe fn new<'a>(input: &'a SystemInput<'a>) -> Result<Self::Item<'a>, &'static str> {
        // Safety. Managed by caller.
        Ok(unsafe { input.world_data.resources().get_ptr::<R>().map(|r| Res { res: &*r }) })
    }
}
