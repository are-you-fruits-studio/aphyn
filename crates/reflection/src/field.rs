use aphyn_ffi::{FfiAnyMut, FfiAnyRef, FfiOption, FfiShortTypeInfo, FfiString};

type FnGetErased = for<'a> unsafe extern "C-unwind" fn(*const (), FfiAnyRef<'a>) -> FfiOption<FfiAnyRef<'a>>;
type FnGetMutErased = for<'a> unsafe extern "C-unwind" fn(*const (), FfiAnyMut<'a>) -> FfiOption<FfiAnyMut<'a>>;

#[repr(C)]
pub struct ReflectField {
    name: FfiString,
    owner_type_info: FfiShortTypeInfo,
    dst_type_info: FfiShortTypeInfo,
    getter: *const (),
    getter_mut: *const (),
    fn_get: FnGetErased,
    fn_get_mut: FnGetMutErased,
}

unsafe impl Send for ReflectField {}
unsafe impl Sync for ReflectField {}

impl ReflectField {
    pub fn new<T: 'static, F: 'static>(name: &str, getter: fn(&T) -> Option<&F>, getter_mut: fn(&mut T) -> Option<&mut F>) -> Self {
        unsafe extern "C-unwind" fn ffi_get<'a, T: 'static, F: 'static>(
            getter: *const (),
            value: FfiAnyRef<'a>,
        ) -> FfiOption<FfiAnyRef<'a>> {
            let getter: fn(&T) -> Option<&F> = unsafe { std::mem::transmute(getter) };

            match any_ref_into::<T>(value) {
                Some(value) => getter(value).map(FfiAnyRef::new).into(),
                None => FfiOption::None,
            }
        }
        unsafe extern "C-unwind" fn ffi_get_mut<'a, T: 'static, F: 'static>(
            getter_mut: *const (),
            value: FfiAnyMut<'a>,
        ) -> FfiOption<FfiAnyMut<'a>> {
            let getter_mut: fn(&mut T) -> Option<&mut F> = unsafe { std::mem::transmute(getter_mut) };

            match any_mut_into::<T>(value) {
                Some(value) => getter_mut(value).map(FfiAnyMut::new).into(),
                None => FfiOption::None,
            }
        }

        Self {
            name: name.into(),
            owner_type_info: FfiShortTypeInfo::of::<T>(),
            dst_type_info: FfiShortTypeInfo::of::<F>(),
            getter: getter as *const (),
            getter_mut: getter_mut as *const (),
            fn_get: ffi_get::<T, F>,
            fn_get_mut: ffi_get_mut::<T, F>,
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn owner_type_info(&self) -> &FfiShortTypeInfo {
        &self.owner_type_info
    }

    pub fn owner_type_name(&self) -> &'static str {
        self.owner_type_info.name()
    }

    pub fn dst_type_info(&self) -> &FfiShortTypeInfo {
        &self.dst_type_info
    }

    pub fn dst_type_name(&self) -> &'static str {
        self.dst_type_info.name()
    }

    pub fn get<'a>(&self, value: FfiAnyRef<'a>) -> Option<FfiAnyRef<'a>> {
        unsafe { (self.fn_get)(self.getter, value) }.into_option()
    }

    pub fn get_mut<'a>(&self, value: FfiAnyMut<'a>) -> Option<FfiAnyMut<'a>> {
        unsafe { (self.fn_get_mut)(self.getter_mut, value) }.into_option()
    }
}

impl std::fmt::Debug for ReflectField {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ReflectField")
            .field("name", &self.name())
            .field("owner", &self.owner_type_name())
            .field("dst", &self.dst_type_name())
            .finish()
    }
}

pub(crate) fn any_ref_into<'a, T: 'static>(value: FfiAnyRef<'a>) -> Option<&'a T> {
    value
        .type_info()
        .short()
        .does_match::<T>()
        .then(|| unsafe { &*(value.ptr() as *const T) })
}

pub(crate) fn any_mut_into<'a, T: 'static>(value: FfiAnyMut<'a>) -> Option<&'a mut T> {
    value
        .type_info()
        .short()
        .does_match::<T>()
        .then(|| unsafe { &mut *(value.ptr() as *mut T) })
}
