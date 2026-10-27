use std::ops::{Deref, DerefMut};

use crate::*;

pub struct ResMut<'e, R: Resource> {
    res: &'e mut R,
}

impl<'e, R: Resource> Deref for ResMut<'e, R> {
    type Target = R;

    fn deref(&self) -> &Self::Target {
        self.res
    }
}

impl<'e, R: Resource> DerefMut for ResMut<'e, R> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.res
    }
}

unsafe impl<'e, R: Resource> SystemParam for ResMut<'e, R> {
    type Item<'b> = ResMut<'b, R>;

    fn fill_data_usage(usage: &mut DataUsageBuilder, types: &TypesRegistryCache) {
        usage.world().resources().add_type(DataUsageEntry {
            type_id: types.get_or_register::<R>(),
            details: DataUsageDetails {
                is_mut: true,
                is_required: true,
            },
        });
    }

    unsafe fn new<'a>(input: &'a SystemInput<'a>) -> Result<Self::Item<'a>, &'static str> {
        Ok(ResMut {
            // Safety. Managed by caller.
            res: unsafe { &mut *input.world_data.resources().get_ptr::<R>().ok_or("Resource missing.")? },
        })
    }
}

unsafe impl<'e, R: Resource> SystemParam for Option<ResMut<'e, R>> {
    type Item<'b> = Option<ResMut<'b, R>>;

    fn fill_data_usage(usage: &mut DataUsageBuilder, types: &TypesRegistryCache) {
        usage.world().resources().add_type(DataUsageEntry {
            type_id: types.get_or_register::<R>(),
            details: DataUsageDetails {
                is_mut: true,
                is_required: false,
            },
        });
    }

    unsafe fn new<'a>(input: &'a SystemInput<'a>) -> Result<Self::Item<'a>, &'static str> {
        // Safety. Managed by caller.
        Ok(unsafe { input.world_data.resources().get_ptr::<R>().map(|r| ResMut { res: &mut *r }) })
    }
}
