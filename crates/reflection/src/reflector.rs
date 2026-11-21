use std::marker::PhantomData;

use aphyn_ffi::{FfiAny, FfiAnyMut, FfiAnyRef, FfiArc, FfiIndexMap, FfiOption, FfiShortTypeInfo, FfiString};

use crate::{ReflectField, any_mut_into, any_ref_into};

#[repr(C)]
#[derive(Debug)]
pub enum Reflector {
    Value(ValueReflector),
    Struct(StructReflector),
    Enum(EnumReflector),
    Collection(CollectionReflector),
}

impl Reflector {
    pub fn type_info(&self) -> &FfiShortTypeInfo {
        match self {
            Reflector::Value(r) => r.type_info(),
            Reflector::Struct(r) => r.type_info(),
            Reflector::Enum(r) => r.type_info(),
            Reflector::Collection(r) => r.type_info(),
        }
    }

    pub fn type_name(&self) -> &'static str {
        self.type_info().name()
    }

    pub fn as_value(&self) -> Option<&ValueReflector> {
        match self {
            Reflector::Value(r) => Some(r),
            _ => None,
        }
    }

    pub fn as_struct(&self) -> Option<&StructReflector> {
        match self {
            Reflector::Struct(r) => Some(r),
            _ => None,
        }
    }

    pub fn as_enum(&self) -> Option<&EnumReflector> {
        match self {
            Reflector::Enum(r) => Some(r),
            _ => None,
        }
    }

    pub fn as_collection(&self) -> Option<&CollectionReflector> {
        match self {
            Reflector::Collection(r) => Some(r),
            _ => None,
        }
    }
}

//

// todo: remove value-reflector. It seems useless
#[repr(C)]
#[derive(Debug)]
pub struct ValueReflector {
    type_info: FfiShortTypeInfo,
}

impl ValueReflector {
    pub fn of<T: 'static>() -> Reflector {
        Reflector::Value(Self {
            type_info: FfiShortTypeInfo::of::<T>(),
        })
    }

    pub fn type_info(&self) -> &FfiShortTypeInfo {
        &self.type_info
    }
}

//

#[repr(C)]
#[derive(Debug)]
pub struct StructReflector {
    type_info: FfiShortTypeInfo,
    fields: FfiIndexMap<FfiString, FfiArc<ReflectField>>,
}

impl StructReflector {
    pub fn type_info(&self) -> &FfiShortTypeInfo {
        &self.type_info
    }

    pub fn field(&self, name: &str) -> Option<&FfiArc<ReflectField>> {
        self.fields.get(name)
    }

    pub fn field_by_idx(&self, idx: usize) -> Option<&FfiArc<ReflectField>> {
        self.fields.get_by_idx(idx as u64).map(|(_, f)| f)
    }

    pub fn fields(&self) -> impl Iterator<Item = &FfiArc<ReflectField>> {
        self.fields.values()
    }

    pub fn fields_count(&self) -> usize {
        self.fields.len() as usize
    }
}

pub struct StructReflectorBuilder<T> {
    fields: FfiIndexMap<FfiString, FfiArc<ReflectField>>,
    _phantom: PhantomData<fn() -> T>,
}

impl<T: 'static> StructReflectorBuilder<T> {
    pub fn new() -> Self {
        Self {
            fields: FfiIndexMap::new(),
            _phantom: PhantomData,
        }
    }

    pub fn field<F: 'static>(mut self, name: &str, getter: fn(&T) -> Option<&F>, getter_mut: fn(&mut T) -> Option<&mut F>) -> Self {
        self.fields
            .insert(name.into(), FfiArc::new(ReflectField::new(name, getter, getter_mut)));
        self
    }

    pub fn build(self) -> Reflector {
        Reflector::Struct(StructReflector {
            type_info: FfiShortTypeInfo::of::<T>(),
            fields: self.fields,
        })
    }
}

impl<T: 'static> Default for StructReflectorBuilder<T> {
    fn default() -> Self {
        Self::new()
    }
}

//

type FnIsActiveErased = unsafe extern "C-unwind" fn(*const (), FfiAnyRef) -> bool;

#[repr(C)]
pub struct EnumVariantReflector {
    name: FfiString,
    fields: StructReflector,
    is_active: *const (),
    fn_is_active: FnIsActiveErased,
}

unsafe impl Send for EnumVariantReflector {}
unsafe impl Sync for EnumVariantReflector {}

impl EnumVariantReflector {
    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn is_active(&self, value: FfiAnyRef) -> bool {
        unsafe { (self.fn_is_active)(self.is_active, value) }
    }

    pub fn field(&self, name: &str) -> Option<&FfiArc<ReflectField>> {
        self.fields.field(name)
    }

    pub fn field_by_idx(&self, idx: usize) -> Option<&FfiArc<ReflectField>> {
        self.fields.field_by_idx(idx)
    }

    pub fn fields(&self) -> impl Iterator<Item = &FfiArc<ReflectField>> {
        self.fields.fields()
    }

    pub fn fields_count(&self) -> usize {
        self.fields.fields_count()
    }
}

impl std::fmt::Debug for EnumVariantReflector {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EnumVariantReflector")
            .field("name", &self.name())
            .field("fields", &self.fields.fields)
            .finish()
    }
}

#[repr(C)]
#[derive(Debug)]
pub struct EnumReflector {
    type_info: FfiShortTypeInfo,
    variants: FfiIndexMap<FfiString, FfiArc<EnumVariantReflector>>,
}

impl EnumReflector {
    pub fn type_info(&self) -> &FfiShortTypeInfo {
        &self.type_info
    }

    pub fn variant(&self, name: &str) -> Option<&FfiArc<EnumVariantReflector>> {
        self.variants.get(name)
    }

    pub fn variant_by_idx(&self, idx: usize) -> Option<&FfiArc<EnumVariantReflector>> {
        self.variants.get_by_idx(idx as u64).map(|(_, v)| v)
    }

    pub fn variants(&self) -> impl Iterator<Item = &FfiArc<EnumVariantReflector>> {
        self.variants.values()
    }

    pub fn variants_count(&self) -> usize {
        self.variants.len() as usize
    }

    pub fn active_variant(&self, value: FfiAnyRef) -> Option<&FfiArc<EnumVariantReflector>> {
        self.variants().find(|v| v.is_active(value))
    }
}

pub struct EnumReflectorBuilder<T> {
    variants: FfiIndexMap<FfiString, FfiArc<EnumVariantReflector>>,
    _phantom: PhantomData<fn() -> T>,
}

impl<T: 'static> EnumReflectorBuilder<T> {
    pub fn new() -> Self {
        Self {
            variants: FfiIndexMap::new(),
            _phantom: PhantomData,
        }
    }

    pub fn variant(
        mut self,
        name: &str,
        is_active: fn(&T) -> bool,
        fields: impl FnOnce(StructReflectorBuilder<T>) -> StructReflectorBuilder<T>,
    ) -> Self {
        unsafe extern "C-unwind" fn ffi_is_active<T: 'static>(is_active: *const (), value: FfiAnyRef) -> bool {
            let is_active: fn(&T) -> bool = unsafe { std::mem::transmute(is_active) };

            any_ref_into::<T>(value).is_some_and(is_active)
        }

        let Reflector::Struct(fields) = fields(StructReflectorBuilder::new()).build() else {
            unreachable!()
        };

        let variant = EnumVariantReflector {
            name: name.into(),
            fields,
            is_active: is_active as *const (),
            fn_is_active: ffi_is_active::<T>,
        };

        self.variants.insert(name.into(), FfiArc::new(variant));
        self
    }

    pub fn build(self) -> Reflector {
        Reflector::Enum(EnumReflector {
            type_info: FfiShortTypeInfo::of::<T>(),
            variants: self.variants,
        })
    }
}

impl<T: 'static> Default for EnumReflectorBuilder<T> {
    fn default() -> Self {
        Self::new()
    }
}

//

type FnLenErased = unsafe extern "C-unwind" fn(*const (), FfiAnyRef) -> FfiOption<u64>;
type FnElementErased = for<'a> unsafe extern "C-unwind" fn(*const (), FfiAnyRef<'a>, u64) -> FfiOption<FfiAnyRef<'a>>;
type FnElementMutErased = for<'a> unsafe extern "C-unwind" fn(*const (), FfiAnyMut<'a>, u64) -> FfiOption<FfiAnyMut<'a>>;
// returns the element back when it was not inserted
type FnInsertErased = unsafe extern "C-unwind" fn(*const (), FfiAnyMut, u64, FfiAny) -> FfiOption<FfiAny>;
type FnRemoveErased = unsafe extern "C-unwind" fn(*const (), FfiAnyMut, u64) -> FfiOption<FfiAny>;

#[repr(C)]
#[derive(Clone, Copy)]
struct ErasedFn<F> {
    func: *const (),
    trampoline: F,
}

#[repr(C)]
pub struct CollectionReflector {
    type_info: FfiShortTypeInfo,
    element_type_info: FfiShortTypeInfo,
    len: ErasedFn<FnLenErased>,
    get: ErasedFn<FnElementErased>,
    get_mut: ErasedFn<FnElementMutErased>,
    insert: FfiOption<ErasedFn<FnInsertErased>>,
    remove: FfiOption<ErasedFn<FnRemoveErased>>,
}

unsafe impl Send for CollectionReflector {}
unsafe impl Sync for CollectionReflector {}

impl CollectionReflector {
    pub fn type_info(&self) -> &FfiShortTypeInfo {
        &self.type_info
    }

    pub fn element_type_info(&self) -> &FfiShortTypeInfo {
        &self.element_type_info
    }

    pub fn element_type_name(&self) -> &'static str {
        self.element_type_info.name()
    }

    pub fn is_resizable(&self) -> bool {
        self.insert.is_some() && self.remove.is_some()
    }

    pub fn len(&self, value: FfiAnyRef) -> Option<usize> {
        unsafe { (self.len.trampoline)(self.len.func, value) }
            .into_option()
            .map(|l| l as usize)
    }

    pub fn get<'a>(&self, value: FfiAnyRef<'a>, idx: usize) -> Option<FfiAnyRef<'a>> {
        unsafe { (self.get.trampoline)(self.get.func, value, idx as u64) }.into_option()
    }

    pub fn get_mut<'a>(&self, value: FfiAnyMut<'a>, idx: usize) -> Option<FfiAnyMut<'a>> {
        unsafe { (self.get_mut.trampoline)(self.get_mut.func, value, idx as u64) }.into_option()
    }

    // Err gives the element back: the collection is fixed-size, a type mismatches or idx > len
    pub fn insert(&self, value: FfiAnyMut, idx: usize, element: FfiAny) -> Result<(), FfiAny> {
        let FfiOption::Some(insert) = self.insert else {
            return Err(element);
        };

        match unsafe { (insert.trampoline)(insert.func, value, idx as u64, element) } {
            FfiOption::Some(element) => Err(element),
            FfiOption::None => Ok(()),
        }
    }

    pub fn remove(&self, value: FfiAnyMut, idx: usize) -> Option<FfiAny> {
        let FfiOption::Some(remove) = self.remove else {
            return None;
        };

        unsafe { (remove.trampoline)(remove.func, value, idx as u64) }.into_option()
    }
}

impl std::fmt::Debug for CollectionReflector {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CollectionReflector")
            .field("type", &self.type_info.name())
            .field("element", &self.element_type_name())
            .field("is_resizable", &self.is_resizable())
            .finish()
    }
}

pub struct CollectionReflectorBuilder<C, E> {
    len: ErasedFn<FnLenErased>,
    get: ErasedFn<FnElementErased>,
    get_mut: ErasedFn<FnElementMutErased>,
    insert: FfiOption<ErasedFn<FnInsertErased>>,
    remove: FfiOption<ErasedFn<FnRemoveErased>>,
    _phantom: PhantomData<fn() -> (C, E)>,
}

impl<C: 'static, E: 'static> CollectionReflectorBuilder<C, E> {
    pub fn new(len: fn(&C) -> usize, get: fn(&C, usize) -> Option<&E>, get_mut: fn(&mut C, usize) -> Option<&mut E>) -> Self {
        unsafe extern "C-unwind" fn ffi_len<C: 'static>(len: *const (), value: FfiAnyRef) -> FfiOption<u64> {
            let len: fn(&C) -> usize = unsafe { std::mem::transmute(len) };

            any_ref_into::<C>(value).map(|c| len(c) as u64).into()
        }
        unsafe extern "C-unwind" fn ffi_get<'a, C: 'static, E: 'static>(
            get: *const (),
            value: FfiAnyRef<'a>,
            idx: u64,
        ) -> FfiOption<FfiAnyRef<'a>> {
            let get: fn(&C, usize) -> Option<&E> = unsafe { std::mem::transmute(get) };

            any_ref_into::<C>(value)
                .and_then(|c| get(c, idx as usize))
                .map(FfiAnyRef::new)
                .into()
        }
        unsafe extern "C-unwind" fn ffi_get_mut<'a, C: 'static, E: 'static>(
            get_mut: *const (),
            value: FfiAnyMut<'a>,
            idx: u64,
        ) -> FfiOption<FfiAnyMut<'a>> {
            let get_mut: fn(&mut C, usize) -> Option<&mut E> = unsafe { std::mem::transmute(get_mut) };

            any_mut_into::<C>(value)
                .and_then(|c| get_mut(c, idx as usize))
                .map(FfiAnyMut::new)
                .into()
        }

        Self {
            len: ErasedFn {
                func: len as *const (),
                trampoline: ffi_len::<C>,
            },
            get: ErasedFn {
                func: get as *const (),
                trampoline: ffi_get::<C, E>,
            },
            get_mut: ErasedFn {
                func: get_mut as *const (),
                trampoline: ffi_get_mut::<C, E>,
            },
            insert: FfiOption::None,
            remove: FfiOption::None,
            _phantom: PhantomData,
        }
    }

    // the insert fn gives the element back when idx is out of bounds
    pub fn insert(mut self, insert: fn(&mut C, usize, E) -> Result<(), E>) -> Self {
        unsafe extern "C-unwind" fn ffi_insert<C: 'static, E: 'static>(
            insert: *const (),
            value: FfiAnyMut,
            idx: u64,
            element: FfiAny,
        ) -> FfiOption<FfiAny> {
            let insert: fn(&mut C, usize, E) -> Result<(), E> = unsafe { std::mem::transmute(insert) };

            let Some(collection) = any_mut_into::<C>(value) else {
                return FfiOption::Some(element);
            };
            if !element.type_info().short().does_match::<E>() {
                return FfiOption::Some(element);
            }
            let element = element.downcast::<E>().unwrap().into_inner();

            match insert(collection, idx as usize, element) {
                Ok(()) => FfiOption::None,
                Err(element) => FfiOption::Some(FfiAny::new(element)),
            }
        }

        self.insert = FfiOption::Some(ErasedFn {
            func: insert as *const (),
            trampoline: ffi_insert::<C, E>,
        });
        self
    }

    pub fn remove(mut self, remove: fn(&mut C, usize) -> Option<E>) -> Self {
        unsafe extern "C-unwind" fn ffi_remove<C: 'static, E: 'static>(
            remove: *const (),
            value: FfiAnyMut,
            idx: u64,
        ) -> FfiOption<FfiAny> {
            let remove: fn(&mut C, usize) -> Option<E> = unsafe { std::mem::transmute(remove) };

            any_mut_into::<C>(value)
                .and_then(|c| remove(c, idx as usize))
                .map(FfiAny::new)
                .into()
        }

        self.remove = FfiOption::Some(ErasedFn {
            func: remove as *const (),
            trampoline: ffi_remove::<C, E>,
        });
        self
    }

    pub fn build(self) -> Reflector {
        Reflector::Collection(CollectionReflector {
            type_info: FfiShortTypeInfo::of::<C>(),
            element_type_info: FfiShortTypeInfo::of::<E>(),
            len: self.len,
            get: self.get,
            get_mut: self.get_mut,
            insert: self.insert,
            remove: self.remove,
        })
    }
}
