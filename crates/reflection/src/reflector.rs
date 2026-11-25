use std::marker::PhantomData;

use aphyn_any::{AnyBox, AnyBoxCtx, AnyMut, AnyRef, AnyTypeId, TypesRegistryCache};
use aphyn_ffi::{FfiArc, FfiExtendedTypeInfo, FfiIndexMap, FfiOption, FfiShortTypeInfo, FfiSliceRef, FfiString, FfiVec};

use crate::{
    ReflectField, ReflectVariantFields, any_mut_into, any_ref_into,
    factories::{ErasedFn, ReflectFactories},
    field::ReflectType,
};

#[repr(C)]
#[derive(Clone, Debug)]
pub enum Reflector {
    Value(ValueReflector),
    Struct(StructReflector),
    Enum(EnumReflector),
    List(ListReflector),
    Map(MapReflector),
}

impl Reflector {
    fn ty(&self) -> &ReflectType {
        match self {
            Reflector::Value(r) => &r.ty,
            Reflector::Struct(r) => &r.ty,
            Reflector::Enum(r) => &r.ty,
            Reflector::List(r) => &r.ty,
            Reflector::Map(r) => &r.ty,
        }
    }

    pub fn type_info(&self) -> &FfiShortTypeInfo {
        self.ty().info.short()
    }

    pub fn extended_type_info(&self) -> &'static FfiExtendedTypeInfo {
        self.ty().info
    }

    pub fn type_id(&self) -> AnyTypeId {
        self.ty().id
    }

    pub fn type_name(&self) -> &'static str {
        self.type_info().name()
    }

    fn factories(&self) -> &ReflectFactories {
        match self {
            Reflector::Value(r) => &r.factories,
            Reflector::Struct(r) => &r.factories,
            Reflector::Enum(r) => &r.factories,
            Reflector::List(r) => &r.factories,
            Reflector::Map(r) => &r.factories,
        }
    }

    pub(crate) fn factories_mut(&mut self) -> &mut ReflectFactories {
        match self {
            Reflector::Value(r) => &mut r.factories,
            Reflector::Struct(r) => &mut r.factories,
            Reflector::Enum(r) => &mut r.factories,
            Reflector::List(r) => &mut r.factories,
            Reflector::Map(r) => &mut r.factories,
        }
    }

    // writes the ids of every type this reflector touches, in the registry the cache belongs to
    pub(crate) fn resolve(&mut self, types: &TypesRegistryCache) {
        match self {
            Reflector::Value(r) => r.ty.resolve(types),
            Reflector::Struct(r) => r.resolve(types),
            Reflector::Enum(r) => {
                r.ty.resolve(types);
                r.variants = r
                    .variants
                    .iter()
                    .map(|(name, variant)| {
                        let mut variant = (**variant).clone();
                        variant.resolve(types);
                        (name.clone(), FfiArc::new(variant))
                    })
                    .collect();
            }
            Reflector::List(r) => {
                r.ty.resolve(types);
                r.element_id = types.any_type_id(r.element);
            }
            Reflector::Map(r) => {
                r.ty.resolve(types);
                r.key_id = types.any_type_id(r.key);
                r.value_id = types.any_type_id(r.value);
            }
        }
    }

    pub fn has_default(&self) -> bool {
        self.factories().has_default()
    }

    pub fn has_clone(&self) -> bool {
        self.factories().has_clone()
    }

    // only the type's own factory; see `reflect_clone` for the default + fill fallback
    pub fn default_value(&self) -> Option<AnyBox> {
        self.factories().default_value(self.ty().box_ctx())
    }

    pub fn clone_value(&self, value: AnyRef) -> Option<AnyBox> {
        self.factories().clone_value(value, self.ty().box_ctx())
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

    pub fn as_list(&self) -> Option<&ListReflector> {
        match self {
            Reflector::List(r) => Some(r),
            _ => None,
        }
    }

    pub fn as_map(&self) -> Option<&MapReflector> {
        match self {
            Reflector::Map(r) => Some(r),
            _ => None,
        }
    }
}

//

#[repr(C)]
#[derive(Clone)]
pub struct ValueReflector {
    ty: ReflectType,
    factories: ReflectFactories,
}

impl ValueReflector {
    pub fn of<T: 'static>() -> Reflector {
        ValueReflectorBuilder::<T>::new().build()
    }

    pub fn type_info(&self) -> &FfiShortTypeInfo {
        self.ty.info.short()
    }
}

impl std::fmt::Debug for ValueReflector {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ValueReflector")
            .field("type", &self.type_info().name())
            .finish()
    }
}

pub struct ValueReflectorBuilder<T> {
    factories: ReflectFactories,
    _phantom: PhantomData<fn() -> T>,
}

impl<T: 'static> ValueReflectorBuilder<T> {
    pub fn new() -> Self {
        Self {
            factories: ReflectFactories::none(),
            _phantom: PhantomData,
        }
    }

    pub fn default_fn(mut self, default: Option<fn() -> T>) -> Self {
        self.factories.set_optional(default, None);
        self
    }

    pub fn clone_fn(mut self, clone: Option<fn(&T) -> T>) -> Self {
        self.factories.set_optional(None, clone);
        self
    }

    pub fn build(self) -> Reflector {
        Reflector::Value(ValueReflector {
            ty: ReflectType::of::<T>(),
            factories: self.factories,
        })
    }
}

impl<T: 'static> Default for ValueReflectorBuilder<T> {
    fn default() -> Self {
        Self::new()
    }
}

//

#[repr(C)]
#[derive(Clone)]
pub struct StructReflector {
    ty: ReflectType,
    fields: FfiIndexMap<FfiString, FfiArc<ReflectField>>,
    factories: ReflectFactories,
}

impl StructReflector {
    pub fn type_info(&self) -> &FfiShortTypeInfo {
        self.ty.info.short()
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

    fn resolve(&mut self, types: &TypesRegistryCache) {
        self.ty.resolve(types);
        self.fields = self
            .fields
            .iter()
            .map(|(name, field)| (name.clone(), FfiArc::new(field.resolved(types))))
            .collect();
    }
}

impl std::fmt::Debug for StructReflector {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StructReflector")
            .field("type", &self.type_info().name())
            .field("fields", &self.fields)
            .finish()
    }
}

pub struct StructReflectorBuilder<T> {
    fields: FfiIndexMap<FfiString, FfiArc<ReflectField>>,
    factories: ReflectFactories,
    _phantom: PhantomData<fn() -> T>,
}

impl<T: 'static> StructReflectorBuilder<T> {
    pub fn new() -> Self {
        Self {
            fields: FfiIndexMap::new(),
            factories: ReflectFactories::none(),
            _phantom: PhantomData,
        }
    }

    pub fn field<F: 'static>(mut self, name: &str, getter: fn(&T) -> Option<&F>, getter_mut: fn(&mut T) -> Option<&mut F>) -> Self {
        self.fields
            .insert(name.into(), FfiArc::new(ReflectField::new(name, getter, getter_mut)));
        self
    }

    pub fn default_fn(mut self, default: Option<fn() -> T>) -> Self {
        self.factories.set_optional(default, None);
        self
    }

    pub fn clone_fn(mut self, clone: Option<fn(&T) -> T>) -> Self {
        self.factories.set_optional(None, clone);
        self
    }

    pub fn build(self) -> Reflector {
        Reflector::Struct(self.build_struct())
    }

    fn build_struct(self) -> StructReflector {
        StructReflector {
            ty: ReflectType::of::<T>(),
            fields: self.fields,
            factories: self.factories,
        }
    }
}

impl<T: 'static> Default for StructReflectorBuilder<T> {
    fn default() -> Self {
        Self::new()
    }
}

//

type FnIsActiveErased = unsafe extern "C-unwind" fn(*const (), AnyRef, AnyTypeId) -> bool;
type FnConstructErased =
    unsafe extern "C-unwind" fn(*const (), FfiVec<AnyBox>, FfiSliceRef<AnyTypeId>, AnyBoxCtx) -> FfiOption<AnyBox>;

#[repr(C)]
#[derive(Clone)]
pub struct EnumVariantReflector {
    name: FfiString,
    fields: StructReflector,
    field_ids: FfiVec<AnyTypeId>,
    is_active: ErasedFn<FnIsActiveErased>,
    construct: ErasedFn<FnConstructErased>,
}

unsafe impl Send for EnumVariantReflector {}
unsafe impl Sync for EnumVariantReflector {}

impl EnumVariantReflector {
    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn is_active(&self, value: AnyRef) -> bool {
        unsafe { (self.is_active.trampoline)(self.is_active.func, value, self.fields.ty.id) }
    }

    // builds the variant from its field values in field order; None on a count or type mismatch
    pub fn construct(&self, fields: FfiVec<AnyBox>) -> Option<AnyBox> {
        unsafe {
            (self.construct.trampoline)(
                self.construct.func,
                fields,
                FfiSliceRef::from_slice(self.field_ids.as_slice()),
                self.fields.ty.box_ctx(),
            )
        }
        .into_option()
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

    fn resolve(&mut self, types: &TypesRegistryCache) {
        self.fields.resolve(types);
        self.field_ids = self.fields.fields().map(|f| f.dst_type_id()).collect();
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
#[derive(Clone)]
pub struct EnumReflector {
    ty: ReflectType,
    variants: FfiIndexMap<FfiString, FfiArc<EnumVariantReflector>>,
    factories: ReflectFactories,
}

impl EnumReflector {
    pub fn type_info(&self) -> &FfiShortTypeInfo {
        self.ty.info.short()
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

    pub fn active_variant(&self, value: AnyRef) -> Option<&FfiArc<EnumVariantReflector>> {
        self.variants().find(|v| v.is_active(value))
    }
}

impl std::fmt::Debug for EnumReflector {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EnumReflector")
            .field("type", &self.type_info().name())
            .field("variants", &self.variants)
            .finish()
    }
}

pub struct EnumReflectorBuilder<T> {
    variants: FfiIndexMap<FfiString, FfiArc<EnumVariantReflector>>,
    factories: ReflectFactories,
    _phantom: PhantomData<fn() -> T>,
}

impl<T: 'static> EnumReflectorBuilder<T> {
    pub fn new() -> Self {
        Self {
            variants: FfiIndexMap::new(),
            factories: ReflectFactories::none(),
            _phantom: PhantomData,
        }
    }

    // `construct` takes the field values in the order `fields` declares them (see `ReflectVariantFields::take`)
    pub fn variant(
        mut self,
        name: &str,
        is_active: fn(&T) -> bool,
        fields: impl FnOnce(StructReflectorBuilder<T>) -> StructReflectorBuilder<T>,
        construct: fn(&mut ReflectVariantFields) -> Option<T>,
    ) -> Self {
        unsafe extern "C-unwind" fn ffi_is_active<T: 'static>(is_active: *const (), value: AnyRef, id: AnyTypeId) -> bool {
            let is_active: fn(&T) -> bool = unsafe { std::mem::transmute(is_active) };

            unsafe { any_ref_into::<T>(value, id) }.is_some_and(is_active)
        }
        unsafe extern "C-unwind" fn ffi_construct<T: 'static>(
            construct: *const (),
            fields: FfiVec<AnyBox>,
            field_ids: FfiSliceRef<AnyTypeId>,
            ctx: AnyBoxCtx,
        ) -> FfiOption<AnyBox> {
            let construct: fn(&mut ReflectVariantFields) -> Option<T> = unsafe { std::mem::transmute(construct) };

            let mut fields = ReflectVariantFields::new(fields.into_iter().collect(), field_ids.into_slice());

            construct(&mut fields).map(|v| unsafe { AnyBox::from_typed(v, ctx) }).into()
        }

        let fields = fields(StructReflectorBuilder::new()).build_struct();

        let variant = EnumVariantReflector {
            name: name.into(),
            field_ids: fields.fields().map(|f| f.dst_type_id()).collect(),
            fields,
            is_active: ErasedFn {
                func: is_active as *const (),
                trampoline: ffi_is_active::<T>,
            },
            construct: ErasedFn {
                func: construct as *const (),
                trampoline: ffi_construct::<T>,
            },
        };

        self.variants.insert(name.into(), FfiArc::new(variant));
        self
    }

    pub fn default_fn(mut self, default: Option<fn() -> T>) -> Self {
        self.factories.set_optional(default, None);
        self
    }

    pub fn clone_fn(mut self, clone: Option<fn(&T) -> T>) -> Self {
        self.factories.set_optional(None, clone);
        self
    }

    pub fn build(self) -> Reflector {
        Reflector::Enum(EnumReflector {
            ty: ReflectType::of::<T>(),
            variants: self.variants,
            factories: self.factories,
        })
    }
}

impl<T: 'static> Default for EnumReflectorBuilder<T> {
    fn default() -> Self {
        Self::new()
    }
}

//

type FnLenErased = unsafe extern "C-unwind" fn(*const (), AnyRef, AnyTypeId) -> FfiOption<u64>;

fn erased_len<C: 'static>(len: fn(&C) -> usize) -> ErasedFn<FnLenErased> {
    unsafe extern "C-unwind" fn ffi_len<C: 'static>(len: *const (), value: AnyRef, id: AnyTypeId) -> FfiOption<u64> {
        let len: fn(&C) -> usize = unsafe { std::mem::transmute(len) };

        unsafe { any_ref_into::<C>(value, id) }.map(|c| len(c) as u64).into()
    }

    ErasedFn {
        func: len as *const (),
        trampoline: ffi_len::<C>,
    }
}

//

type FnElementErased =
    for<'a> unsafe extern "C-unwind" fn(*const (), AnyRef<'a>, u64, AnyTypeId, AnyTypeId) -> FfiOption<AnyRef<'a>>;
type FnElementMutErased =
    for<'a> unsafe extern "C-unwind" fn(*const (), AnyMut<'a>, u64, AnyTypeId, AnyTypeId) -> FfiOption<AnyMut<'a>>;
// returns the element back when it was not inserted
type FnInsertErased = unsafe extern "C-unwind" fn(*const (), AnyMut, u64, AnyBox, AnyTypeId, AnyBoxCtx) -> FfiOption<AnyBox>;
type FnRemoveErased = unsafe extern "C-unwind" fn(*const (), AnyMut, u64, AnyTypeId, AnyBoxCtx) -> FfiOption<AnyBox>;

#[repr(C)]
#[derive(Clone, Copy)]
struct ListResizeFns {
    insert: ErasedFn<FnInsertErased>,
    remove: ErasedFn<FnRemoveErased>,
}

#[repr(C)]
#[derive(Clone)]
pub struct ListReflector {
    ty: ReflectType,
    element: &'static FfiExtendedTypeInfo,
    element_id: AnyTypeId,
    len: ErasedFn<FnLenErased>,
    get: ErasedFn<FnElementErased>,
    get_mut: ErasedFn<FnElementMutErased>,
    resize: FfiOption<ListResizeFns>,
    factories: ReflectFactories,
}

unsafe impl Send for ListReflector {}
unsafe impl Sync for ListReflector {}

impl ListReflector {
    pub fn type_info(&self) -> &FfiShortTypeInfo {
        self.ty.info.short()
    }

    pub fn element_type_info(&self) -> &FfiShortTypeInfo {
        self.element.short()
    }

    pub fn element_type_name(&self) -> &'static str {
        self.element.short().name()
    }

    pub fn element_type_id(&self) -> AnyTypeId {
        self.element_id
    }

    pub fn element_extended_type_info(&self) -> &'static FfiExtendedTypeInfo {
        self.element
    }

    pub fn is_rigid(&self) -> bool {
        self.resize.is_none()
    }

    pub fn len(&self, value: AnyRef) -> Option<usize> {
        unsafe { (self.len.trampoline)(self.len.func, value, self.ty.id) }
            .into_option()
            .map(|l| l as usize)
    }

    pub fn get<'a>(&self, value: AnyRef<'a>, idx: usize) -> Option<AnyRef<'a>> {
        unsafe { (self.get.trampoline)(self.get.func, value, idx as u64, self.ty.id, self.element_id) }.into_option()
    }

    pub fn get_mut<'a>(&self, value: AnyMut<'a>, idx: usize) -> Option<AnyMut<'a>> {
        unsafe { (self.get_mut.trampoline)(self.get_mut.func, value, idx as u64, self.ty.id, self.element_id) }.into_option()
    }

    // Err gives the element back: the list is rigid, a type mismatches or idx > len
    pub fn insert(&self, value: AnyMut, idx: usize, element: AnyBox) -> Result<(), AnyBox> {
        let FfiOption::Some(ListResizeFns { insert, .. }) = self.resize else {
            return Err(element);
        };

        let element_ctx = self.ty.box_ctx_for(self.element_id);

        match unsafe { (insert.trampoline)(insert.func, value, idx as u64, element, self.ty.id, element_ctx) } {
            FfiOption::Some(element) => Err(element),
            FfiOption::None => Ok(()),
        }
    }

    pub fn remove(&self, value: AnyMut, idx: usize) -> Option<AnyBox> {
        let FfiOption::Some(ListResizeFns { remove, .. }) = self.resize else {
            return None;
        };

        let element_ctx = self.ty.box_ctx_for(self.element_id);

        unsafe { (remove.trampoline)(remove.func, value, idx as u64, self.ty.id, element_ctx) }.into_option()
    }
}

impl std::fmt::Debug for ListReflector {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ListReflector")
            .field("type", &self.type_info().name())
            .field("element", &self.element_type_name())
            .field("is_rigid", &self.is_rigid())
            .finish()
    }
}

pub struct ListReflectorBuilder<C, E> {
    len: ErasedFn<FnLenErased>,
    get: ErasedFn<FnElementErased>,
    get_mut: ErasedFn<FnElementMutErased>,
    resize: FfiOption<ListResizeFns>,
    factories: ReflectFactories,
    _phantom: PhantomData<fn() -> (C, E)>,
}

impl<C: 'static, E: 'static> ListReflectorBuilder<C, E> {
    // the insert fn gives the element back when idx is out of bounds
    pub fn new(
        len: fn(&C) -> usize,
        get: fn(&C, usize) -> Option<&E>,
        get_mut: fn(&mut C, usize) -> Option<&mut E>,
        insert_remove: Option<(fn(&mut C, usize, E) -> Result<(), E>, fn(&mut C, usize) -> Option<E>)>,
    ) -> Self {
        unsafe extern "C-unwind" fn ffi_get<'a, C: 'static, E: 'static>(
            get: *const (),
            value: AnyRef<'a>,
            idx: u64,
            list_id: AnyTypeId,
            element_id: AnyTypeId,
        ) -> FfiOption<AnyRef<'a>> {
            let get: fn(&C, usize) -> Option<&E> = unsafe { std::mem::transmute(get) };

            unsafe { any_ref_into::<C>(value, list_id) }
                .and_then(|c| get(c, idx as usize))
                .map(|e| unsafe { AnyRef::from_typed(e, element_id) })
                .into()
        }
        unsafe extern "C-unwind" fn ffi_get_mut<'a, C: 'static, E: 'static>(
            get_mut: *const (),
            value: AnyMut<'a>,
            idx: u64,
            list_id: AnyTypeId,
            element_id: AnyTypeId,
        ) -> FfiOption<AnyMut<'a>> {
            let get_mut: fn(&mut C, usize) -> Option<&mut E> = unsafe { std::mem::transmute(get_mut) };

            unsafe { any_mut_into::<C>(value, list_id) }
                .and_then(|c| get_mut(c, idx as usize))
                .map(|e| unsafe { AnyMut::from_typed(e, element_id) })
                .into()
        }
        unsafe extern "C-unwind" fn ffi_insert<C: 'static, E: 'static>(
            insert: *const (),
            value: AnyMut,
            idx: u64,
            element: AnyBox,
            list_id: AnyTypeId,
            element_ctx: AnyBoxCtx,
        ) -> FfiOption<AnyBox> {
            let insert: fn(&mut C, usize, E) -> Result<(), E> = unsafe { std::mem::transmute(insert) };

            let Some(list) = (unsafe { any_mut_into::<C>(value, list_id) }) else {
                return FfiOption::Some(element);
            };
            let element = match unsafe { element.downcast_by_id::<E>(element_ctx.id) } {
                Ok(element) => element,
                Err(element) => return FfiOption::Some(element),
            };

            match insert(list, idx as usize, element) {
                Ok(()) => FfiOption::None,
                Err(element) => FfiOption::Some(unsafe { AnyBox::from_typed(element, element_ctx) }),
            }
        }
        unsafe extern "C-unwind" fn ffi_remove<C: 'static, E: 'static>(
            remove: *const (),
            value: AnyMut,
            idx: u64,
            list_id: AnyTypeId,
            element_ctx: AnyBoxCtx,
        ) -> FfiOption<AnyBox> {
            let remove: fn(&mut C, usize) -> Option<E> = unsafe { std::mem::transmute(remove) };

            unsafe { any_mut_into::<C>(value, list_id) }
                .and_then(|c| remove(c, idx as usize))
                .map(|e| unsafe { AnyBox::from_typed(e, element_ctx) })
                .into()
        }

        Self {
            len: erased_len(len),
            get: ErasedFn {
                func: get as *const (),
                trampoline: ffi_get::<C, E>,
            },
            get_mut: ErasedFn {
                func: get_mut as *const (),
                trampoline: ffi_get_mut::<C, E>,
            },
            resize: insert_remove
                .map(|(insert, remove)| ListResizeFns {
                    insert: ErasedFn {
                        func: insert as *const (),
                        trampoline: ffi_insert::<C, E>,
                    },
                    remove: ErasedFn {
                        func: remove as *const (),
                        trampoline: ffi_remove::<C, E>,
                    },
                })
                .into(),
            factories: ReflectFactories::none(),
            _phantom: PhantomData,
        }
    }

    pub fn default_fn(mut self, default: Option<fn() -> C>) -> Self {
        self.factories.set_optional(default, None);
        self
    }

    pub fn clone_fn(mut self, clone: Option<fn(&C) -> C>) -> Self {
        self.factories.set_optional(None, clone);
        self
    }

    pub fn build(self) -> Reflector {
        Reflector::List(ListReflector {
            ty: ReflectType::of::<C>(),
            element: const { &FfiExtendedTypeInfo::of::<E>() },
            element_id: AnyTypeId::UNRESOLVED,
            len: self.len,
            get: self.get,
            get_mut: self.get_mut,
            resize: self.resize,
            factories: self.factories,
        })
    }
}

//

#[repr(C)]
struct ErasedMapEntry {
    key: AnyBox,
    value: AnyBox,
}

type FnKeysErased =
    for<'a> unsafe extern "C-unwind" fn(*const (), AnyRef<'a>, AnyTypeId, AnyTypeId) -> FfiOption<FfiVec<AnyRef<'a>>>;
type FnValueErased = for<'a, 'k> unsafe extern "C-unwind" fn(
    *const (),
    AnyRef<'a>,
    AnyRef<'k>,
    AnyTypeId,
    AnyTypeId,
    AnyTypeId,
) -> FfiOption<AnyRef<'a>>;
type FnValueMutErased = for<'a, 'k> unsafe extern "C-unwind" fn(
    *const (),
    AnyMut<'a>,
    AnyRef<'k>,
    AnyTypeId,
    AnyTypeId,
    AnyTypeId,
) -> FfiOption<AnyMut<'a>>;
// returns the entry back when it was not inserted
type FnMapInsertErased =
    unsafe extern "C-unwind" fn(*const (), AnyMut, AnyBox, AnyBox, AnyTypeId, AnyBoxCtx, AnyBoxCtx) -> FfiOption<ErasedMapEntry>;
type FnMapRemoveErased =
    unsafe extern "C-unwind" fn(*const (), AnyMut, AnyRef, AnyTypeId, AnyTypeId, AnyBoxCtx) -> FfiOption<AnyBox>;

#[repr(C)]
#[derive(Clone, Copy)]
struct MapResizeFns {
    insert: ErasedFn<FnMapInsertErased>,
    remove: ErasedFn<FnMapRemoveErased>,
}

#[repr(C)]
#[derive(Clone)]
pub struct MapReflector {
    ty: ReflectType,
    key: &'static FfiExtendedTypeInfo,
    value: &'static FfiExtendedTypeInfo,
    key_id: AnyTypeId,
    value_id: AnyTypeId,
    len: ErasedFn<FnLenErased>,
    keys: ErasedFn<FnKeysErased>,
    get: ErasedFn<FnValueErased>,
    get_mut: ErasedFn<FnValueMutErased>,
    resize: FfiOption<MapResizeFns>,
    factories: ReflectFactories,
}

unsafe impl Send for MapReflector {}
unsafe impl Sync for MapReflector {}

impl MapReflector {
    pub fn type_info(&self) -> &FfiShortTypeInfo {
        self.ty.info.short()
    }

    pub fn key_type_info(&self) -> &FfiShortTypeInfo {
        self.key.short()
    }

    pub fn key_type_name(&self) -> &'static str {
        self.key.short().name()
    }

    pub fn key_type_id(&self) -> AnyTypeId {
        self.key_id
    }

    pub fn key_extended_type_info(&self) -> &'static FfiExtendedTypeInfo {
        self.key
    }

    pub fn value_type_info(&self) -> &FfiShortTypeInfo {
        self.value.short()
    }

    pub fn value_type_name(&self) -> &'static str {
        self.value.short().name()
    }

    pub fn value_type_id(&self) -> AnyTypeId {
        self.value_id
    }

    pub fn value_extended_type_info(&self) -> &'static FfiExtendedTypeInfo {
        self.value
    }

    pub fn is_rigid(&self) -> bool {
        self.resize.is_none()
    }

    pub fn len(&self, map: AnyRef) -> Option<usize> {
        unsafe { (self.len.trampoline)(self.len.func, map, self.ty.id) }
            .into_option()
            .map(|l| l as usize)
    }

    pub fn keys<'a>(&self, map: AnyRef<'a>) -> Option<FfiVec<AnyRef<'a>>> {
        unsafe { (self.keys.trampoline)(self.keys.func, map, self.ty.id, self.key_id) }.into_option()
    }

    pub fn get<'a>(&self, map: AnyRef<'a>, key: AnyRef) -> Option<AnyRef<'a>> {
        unsafe { (self.get.trampoline)(self.get.func, map, key, self.ty.id, self.key_id, self.value_id) }.into_option()
    }

    pub fn get_mut<'a>(&self, map: AnyMut<'a>, key: AnyRef) -> Option<AnyMut<'a>> {
        unsafe { (self.get_mut.trampoline)(self.get_mut.func, map, key, self.ty.id, self.key_id, self.value_id) }.into_option()
    }

    // Err gives the key and value back: the map is rigid, a type mismatches or the key is already present
    pub fn insert(&self, map: AnyMut, key: AnyBox, value: AnyBox) -> Result<(), (AnyBox, AnyBox)> {
        let FfiOption::Some(MapResizeFns { insert, .. }) = self.resize else {
            return Err((key, value));
        };

        let key_ctx = self.ty.box_ctx_for(self.key_id);
        let value_ctx = self.ty.box_ctx_for(self.value_id);

        match unsafe { (insert.trampoline)(insert.func, map, key, value, self.ty.id, key_ctx, value_ctx) } {
            FfiOption::Some(ErasedMapEntry { key, value }) => Err((key, value)),
            FfiOption::None => Ok(()),
        }
    }

    pub fn remove(&self, map: AnyMut, key: AnyRef) -> Option<AnyBox> {
        let FfiOption::Some(MapResizeFns { remove, .. }) = self.resize else {
            return None;
        };

        let value_ctx = self.ty.box_ctx_for(self.value_id);

        unsafe { (remove.trampoline)(remove.func, map, key, self.ty.id, self.key_id, value_ctx) }.into_option()
    }
}

impl std::fmt::Debug for MapReflector {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MapReflector")
            .field("type", &self.type_info().name())
            .field("key", &self.key_type_name())
            .field("value", &self.value_type_name())
            .field("is_rigid", &self.is_rigid())
            .finish()
    }
}

pub struct MapReflectorBuilder<M, K, V> {
    len: ErasedFn<FnLenErased>,
    keys: ErasedFn<FnKeysErased>,
    get: ErasedFn<FnValueErased>,
    get_mut: ErasedFn<FnValueMutErased>,
    resize: FfiOption<MapResizeFns>,
    factories: ReflectFactories,
    _phantom: PhantomData<fn() -> (M, K, V)>,
}

impl<M: 'static, K: 'static, V: 'static> MapReflectorBuilder<M, K, V> {
    // the insert fn gives the key and value back when the key is already present
    // todo: avoid allocating a key snapshot
    pub fn new(
        len: fn(&M) -> usize,
        keys: fn(&M) -> Vec<&K>,
        get: for<'a> fn(&'a M, &K) -> Option<&'a V>,
        get_mut: for<'a> fn(&'a mut M, &K) -> Option<&'a mut V>,
        insert_remove: Option<(fn(&mut M, K, V) -> Result<(), (K, V)>, fn(&mut M, &K) -> Option<V>)>,
    ) -> Self {
        unsafe extern "C-unwind" fn ffi_keys<'a, M: 'static, K: 'static>(
            keys: *const (),
            map: AnyRef<'a>,
            map_id: AnyTypeId,
            key_id: AnyTypeId,
        ) -> FfiOption<FfiVec<AnyRef<'a>>> {
            let keys: fn(&M) -> Vec<&K> = unsafe { std::mem::transmute(keys) };

            unsafe { any_ref_into::<M>(map, map_id) }
                .map(|m| {
                    keys(m)
                        .into_iter()
                        .map(|k| unsafe { AnyRef::from_typed(k, key_id) })
                        .collect()
                })
                .into()
        }
        unsafe extern "C-unwind" fn ffi_get<'a, M: 'static, K: 'static, V: 'static>(
            get: *const (),
            map: AnyRef<'a>,
            key: AnyRef,
            map_id: AnyTypeId,
            key_id: AnyTypeId,
            value_id: AnyTypeId,
        ) -> FfiOption<AnyRef<'a>> {
            let get: for<'m> fn(&'m M, &K) -> Option<&'m V> = unsafe { std::mem::transmute(get) };

            unsafe { any_ref_into::<M>(map, map_id) }
                .zip(unsafe { any_ref_into::<K>(key, key_id) })
                .and_then(|(m, k)| get(m, k))
                .map(|v| unsafe { AnyRef::from_typed(v, value_id) })
                .into()
        }
        unsafe extern "C-unwind" fn ffi_get_mut<'a, M: 'static, K: 'static, V: 'static>(
            get_mut: *const (),
            map: AnyMut<'a>,
            key: AnyRef,
            map_id: AnyTypeId,
            key_id: AnyTypeId,
            value_id: AnyTypeId,
        ) -> FfiOption<AnyMut<'a>> {
            let get_mut: for<'m> fn(&'m mut M, &K) -> Option<&'m mut V> = unsafe { std::mem::transmute(get_mut) };

            unsafe { any_mut_into::<M>(map, map_id) }
                .zip(unsafe { any_ref_into::<K>(key, key_id) })
                .and_then(|(m, k)| get_mut(m, k))
                .map(|v| unsafe { AnyMut::from_typed(v, value_id) })
                .into()
        }
        unsafe extern "C-unwind" fn ffi_insert<M: 'static, K: 'static, V: 'static>(
            insert: *const (),
            map: AnyMut,
            key: AnyBox,
            value: AnyBox,
            map_id: AnyTypeId,
            key_ctx: AnyBoxCtx,
            value_ctx: AnyBoxCtx,
        ) -> FfiOption<ErasedMapEntry> {
            let insert: fn(&mut M, K, V) -> Result<(), (K, V)> = unsafe { std::mem::transmute(insert) };

            let Some(m) = (unsafe { any_mut_into::<M>(map, map_id) }) else {
                return FfiOption::Some(ErasedMapEntry { key, value });
            };
            if !unsafe { key.is_by_id::<K>(key_ctx.id) && value.is_by_id::<V>(value_ctx.id) } {
                return FfiOption::Some(ErasedMapEntry { key, value });
            }
            let key = unsafe { key.downcast_by_id::<K>(key_ctx.id) }.ok().unwrap();
            let value = unsafe { value.downcast_by_id::<V>(value_ctx.id) }.ok().unwrap();

            match insert(m, key, value) {
                Ok(()) => FfiOption::None,
                Err((key, value)) => FfiOption::Some(ErasedMapEntry {
                    key: unsafe { AnyBox::from_typed(key, key_ctx) },
                    value: unsafe { AnyBox::from_typed(value, value_ctx) },
                }),
            }
        }
        unsafe extern "C-unwind" fn ffi_remove<M: 'static, K: 'static, V: 'static>(
            remove: *const (),
            map: AnyMut,
            key: AnyRef,
            map_id: AnyTypeId,
            key_id: AnyTypeId,
            value_ctx: AnyBoxCtx,
        ) -> FfiOption<AnyBox> {
            let remove: fn(&mut M, &K) -> Option<V> = unsafe { std::mem::transmute(remove) };

            unsafe { any_mut_into::<M>(map, map_id) }
                .zip(unsafe { any_ref_into::<K>(key, key_id) })
                .and_then(|(m, k)| remove(m, k))
                .map(|v| unsafe { AnyBox::from_typed(v, value_ctx) })
                .into()
        }

        Self {
            len: erased_len(len),
            keys: ErasedFn {
                func: keys as *const (),
                trampoline: ffi_keys::<M, K>,
            },
            get: ErasedFn {
                func: get as *const (),
                trampoline: ffi_get::<M, K, V>,
            },
            get_mut: ErasedFn {
                func: get_mut as *const (),
                trampoline: ffi_get_mut::<M, K, V>,
            },
            resize: insert_remove
                .map(|(insert, remove)| MapResizeFns {
                    insert: ErasedFn {
                        func: insert as *const (),
                        trampoline: ffi_insert::<M, K, V>,
                    },
                    remove: ErasedFn {
                        func: remove as *const (),
                        trampoline: ffi_remove::<M, K, V>,
                    },
                })
                .into(),
            factories: ReflectFactories::none(),
            _phantom: PhantomData,
        }
    }

    pub fn default_fn(mut self, default: Option<fn() -> M>) -> Self {
        self.factories.set_optional(default, None);
        self
    }

    pub fn clone_fn(mut self, clone: Option<fn(&M) -> M>) -> Self {
        self.factories.set_optional(None, clone);
        self
    }

    pub fn build(self) -> Reflector {
        Reflector::Map(MapReflector {
            ty: ReflectType::of::<M>(),
            key: const { &FfiExtendedTypeInfo::of::<K>() },
            value: const { &FfiExtendedTypeInfo::of::<V>() },
            key_id: AnyTypeId::UNRESOLVED,
            value_id: AnyTypeId::UNRESOLVED,
            len: self.len,
            keys: self.keys,
            get: self.get,
            get_mut: self.get_mut,
            resize: self.resize,
            factories: self.factories,
        })
    }
}
