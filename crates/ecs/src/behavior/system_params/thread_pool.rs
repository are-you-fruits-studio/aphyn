use crate::*;

unsafe impl<'p> SystemParam for ThreadPoolAccess<'p> {
    type Item<'b> = ThreadPoolAccess<'b>;

    fn fill_data_usage(_usage: &mut DataUsageBuilder, _types: &TypesRegistryCache) {}

    unsafe fn new<'a>(input: &'a SystemInput<'a>) -> Result<Self::Item<'a>, &'static str> {
        Ok(input.thread_pool)
    }
}
