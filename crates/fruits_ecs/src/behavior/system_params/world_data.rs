use crate::*;

unsafe impl<'b> SystemParam for WorldDataMut<'b> {
    type Item<'e> = WorldDataMut<'e>;

    fn fill_data_usage(usage: &mut DataUsageBuilder, _types: &TypesRegistryCache) {
        usage.world().add_global(true);
    }

    unsafe fn new<'a>(input: &'a SystemInput<'a>) -> Result<Self::Item<'a>, &'static str> {
        // Safety. Managed by caller
        Ok(unsafe { input.world_data.into_mut() })
    }
}

unsafe impl<'b> SystemParam for WorldDataRef<'b> {
    type Item<'e> = WorldDataRef<'e>;

    fn fill_data_usage(usage: &mut DataUsageBuilder, _types: &TypesRegistryCache) {
        usage.world().add_global(false);
    }

    unsafe fn new<'a>(input: &'a SystemInput<'a>) -> Result<Self::Item<'a>, &'static str> {
        // Safety. Managed by caller
        Ok(unsafe { input.world_data })
    }
}

//

unsafe impl<'b> SystemParam for ResourcesHolderMut<'b> {
    type Item<'e> = ResourcesHolderMut<'e>;

    fn fill_data_usage(usage: &mut DataUsageBuilder, _types: &TypesRegistryCache) {
        usage.world().resources().add_global(true);
    }

    unsafe fn new<'a>(input: &'a SystemInput<'a>) -> Result<Self::Item<'a>, &'static str> {
        // Safety. Managed by caller
        Ok(unsafe { input.world_data.into_mut().into_resources_mut() })
    }
}

unsafe impl<'b> SystemParam for ResourcesHolderRef<'b> {
    type Item<'e> = ResourcesHolderRef<'e>;

    fn fill_data_usage(usage: &mut DataUsageBuilder, _types: &TypesRegistryCache) {
        usage.world().resources().add_global(false);
    }

    unsafe fn new<'a>(input: &'a SystemInput<'a>) -> Result<Self::Item<'a>, &'static str> {
        // Safety. Managed by caller
        Ok(unsafe { input.world_data.resources() })
    }
}

unsafe impl<'b> SystemParam for EntitiesHolderMut<'b> {
    type Item<'e> = EntitiesHolderMut<'e>;

    fn fill_data_usage(usage: &mut DataUsageBuilder, _types: &TypesRegistryCache) {
        usage.world().entities().add_global(true);
    }

    unsafe fn new<'a>(input: &'a SystemInput<'a>) -> Result<Self::Item<'a>, &'static str> {
        // Safety. Managed by caller
        Ok(unsafe { input.world_data.into_mut().into_entities_mut() })
    }
}

unsafe impl<'b> SystemParam for EntitiesHolderRef<'b> {
    type Item<'e> = EntitiesHolderRef<'e>;

    fn fill_data_usage(usage: &mut DataUsageBuilder, _types: &TypesRegistryCache) {
        usage.world().entities().add_global(false);
    }

    unsafe fn new<'a>(input: &'a SystemInput<'a>) -> Result<Self::Item<'a>, &'static str> {
        // Safety. Managed by caller
        Ok(unsafe { input.world_data.entities() })
    }
}

unsafe impl<'b> SystemParam for EventsHolderMut<'b> {
    type Item<'e> = EventsHolderMut<'e>;

    fn fill_data_usage(usage: &mut DataUsageBuilder, _types: &TypesRegistryCache) {
        usage.world().events().add_global(true);
    }

    unsafe fn new<'a>(input: &'a SystemInput<'a>) -> Result<Self::Item<'a>, &'static str> {
        // Safety. Managed by caller
        Ok(unsafe { input.world_data.into_mut().into_events_mut() })
    }
}

unsafe impl<'b> SystemParam for EventsHolderRef<'b> {
    type Item<'e> = EventsHolderRef<'e>;

    fn fill_data_usage(usage: &mut DataUsageBuilder, _types: &TypesRegistryCache) {
        usage.world().events().add_global(false);
    }

    unsafe fn new<'a>(input: &'a SystemInput<'a>) -> Result<Self::Item<'a>, &'static str> {
        // Safety. Managed by caller
        Ok(unsafe { input.world_data.events() })
    }
}