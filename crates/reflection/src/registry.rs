use aphyn_any::{AnyRef, AnyTypeId, TypesRegistryAccessFfi, TypesRegistryCache};
use aphyn_ffi::{FfiArc, FfiExtendedTypeInfo, FfiOption, FfiVec};

use crate::{ReflectField, Reflectable, Reflector};

#[repr(C)]
#[derive(Default)]
pub struct ReflectionRegistryBuilder {
    // indexed by the reflected type's id in `types`
    reflectors: FfiVec<FfiOption<Reflector>>,
    types: FfiOption<TypesRegistryAccessFfi>,
}

impl ReflectionRegistryBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    // `types` is the calling binary's cache of the registry of the world the values live in; every
    // registration must use the same registry
    pub fn register<T: Reflectable>(&mut self, types: &TypesRegistryCache) -> &mut Self {
        self.check_types(types);
        if !self.contains(types.type_id_of::<T>().any_type_id()) {
            self.register_reflector(types, T::reflector());
        }
        self
    }

    // for generic instantiations whose generic impl can't name a default; keeps an existing factory
    pub fn register_with_default<T: Reflectable + Default>(&mut self, types: &TypesRegistryCache) -> &mut Self {
        self.register::<T>(types);
        if let Some(reflector) = self.get_mut(types.get_or_register::<T>())
            && !reflector.has_default()
        {
            reflector.factories_mut().set_default::<T>(T::default);
        }
        self
    }

    // for generic instantiations whose generic impl can't name a clone; keeps an existing factory
    pub fn register_with_clone<T: Reflectable + Clone>(&mut self, types: &TypesRegistryCache) -> &mut Self {
        self.register::<T>(types);
        if let Some(reflector) = self.get_mut(types.get_or_register::<T>())
            && !reflector.has_clone()
        {
            reflector.factories_mut().set_clone::<T>(T::clone);
        }
        self
    }

    pub fn register_with_factories<T: Reflectable + Default + Clone>(&mut self, types: &TypesRegistryCache) -> &mut Self {
        self.register_with_default::<T>(types).register_with_clone::<T>(types)
    }

    // resolves every type the reflector touches to its id in `types`; keeps an already registered reflector
    pub fn register_reflector(&mut self, types: &TypesRegistryCache, mut reflector: Reflector) -> &mut Self {
        self.check_types(types);

        let id = types.any_type_id(reflector.extended_type_info());
        if self.contains(id) {
            return self;
        }

        reflector.resolve(types);

        let idx = id.id();
        if self.reflectors.len() <= idx {
            self.reflectors.resize(idx + 1, FfiOption::None);
        }
        *self.reflectors.get_mut(idx).unwrap() = FfiOption::Some(reflector);
        self
    }

    pub fn contains(&self, id: AnyTypeId) -> bool {
        self.reflectors.get(id.id()).is_some_and(FfiOption::is_some)
    }

    pub fn build(self) -> ReflectionRegistry {
        let reflectors = self
            .reflectors
            .into_iter()
            .map(|r| FfiOption::from_option(r.into_option().map(FfiArc::new)))
            .collect::<FfiVec<FfiOption<FfiArc<Reflector>>>>();
        let len = reflectors.as_slice().iter().filter(|r| r.is_some()).count();

        ReflectionRegistry {
            reflectors: FfiArc::new(reflectors),
            len: len as u64,
            types: self.types,
        }
    }

    fn get_mut(&mut self, id: u64) -> Option<&mut Reflector> {
        self.reflectors.get_mut(id)?.as_mut()
    }

    fn check_types(&mut self, types: &TypesRegistryCache) {
        match self.types.as_ref() {
            Some(registered) => assert!(
                registered.tag() == types.tag(),
                "all reflector registrations must use the same types registry"
            ),
            None => self.types = FfiOption::Some(types.ffi().clone()),
        }
    }
}

#[repr(C)]
#[derive(Clone)]
pub struct ReflectionRegistry {
    // indexed by the reflected type's id in `types`
    reflectors: FfiArc<FfiVec<FfiOption<FfiArc<Reflector>>>>,
    len: u64,
    types: FfiOption<TypesRegistryAccessFfi>,
}

impl ReflectionRegistry {
    // the types registry the reflectors' ids were resolved in; None when nothing was registered
    pub fn types(&self) -> Option<&TypesRegistryAccessFfi> {
        self.types.as_ref()
    }

    // None for an id of another types registry; see `get_for`
    pub fn get(&self, id: AnyTypeId) -> Option<&FfiArc<Reflector>> {
        if !self.types.as_ref().is_some_and(|t| t.tag() == id.tag()) {
            return None;
        }
        self.reflectors.get(id.id())?.as_ref()
    }

    pub fn get_of<T: 'static>(&self, types: &TypesRegistryCache) -> Option<&FfiArc<Reflector>> {
        self.get(types.type_id_of::<T>().any_type_id())
    }

    // the reflector of a value's type, also for values of another world (resolved through the types registry)
    pub fn get_for(&self, value: AnyRef) -> Option<&FfiArc<Reflector>> {
        self.get_for_type(value.type_id(), value.type_info())
    }

    pub fn get_for_type(&self, id: AnyTypeId, info: &'static FfiExtendedTypeInfo) -> Option<&FfiArc<Reflector>> {
        if let Some(reflector) = self.get(id) {
            return Some(reflector);
        }
        let types = self.types.as_ref()?;
        if types.tag() == id.tag() {
            return None;
        }
        self.get(AnyTypeId::new(types.tag(), types.get_by_name(info.short().name())?.id))
    }

    pub fn dst_of(&self, field: &ReflectField) -> Option<&FfiArc<Reflector>> {
        self.get(field.dst_type_id())
    }

    pub fn iter(&self) -> impl Iterator<Item = &FfiArc<Reflector>> {
        self.reflectors.as_slice().iter().filter_map(|r| r.as_ref())
    }

    pub fn len(&self) -> usize {
        self.len as usize
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn missing_dst_types(&self) -> Vec<&'static str> {
        let mut missing = Vec::new();
        let mut check = |id: AnyTypeId, info: &'static FfiExtendedTypeInfo| {
            let name = info.short().name();
            if self.get(id).is_none() && !missing.contains(&name) {
                missing.push(name);
            }
        };

        for reflector in self.iter() {
            match &**reflector {
                Reflector::Value(_) => {}
                Reflector::Struct(r) => r.fields().for_each(|f| check(f.dst_type_id(), f.dst_extended_type_info())),
                Reflector::Enum(r) => r
                    .variants()
                    .flat_map(|v| v.fields())
                    .for_each(|f| check(f.dst_type_id(), f.dst_extended_type_info())),
                Reflector::List(r) => check(r.element_type_id(), r.element_extended_type_info()),
                Reflector::Map(r) => {
                    check(r.key_type_id(), r.key_extended_type_info());
                    check(r.value_type_id(), r.value_extended_type_info());
                }
            }
        }

        missing
    }
}

impl Default for ReflectionRegistry {
    fn default() -> Self {
        ReflectionRegistryBuilder::new().build()
    }
}

impl std::fmt::Debug for ReflectionRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_list().entries(self.iter().map(|r| r.type_name())).finish()
    }
}

#[cfg(test)]
mod tests {
    use aphyn_any::{AnyRef, TypesRegistryAccessFfi, TypesRegistryCache};

    use crate::ReflectionRegistryBuilder;

    fn new_types() -> TypesRegistryCache {
        TypesRegistryCache::new(TypesRegistryAccessFfi::new())
    }

    #[test]
    #[should_panic(expected = "same types registry")]
    fn registering_with_two_registries_panics() {
        let mut builder = ReflectionRegistryBuilder::new();
        builder.register::<u32>(&new_types());
        builder.register::<f32>(&new_types());
    }

    #[test]
    fn lookups_by_id_and_for_values_of_another_world() {
        let types = new_types();
        let other = new_types();
        other.get_or_register::<u8>();

        let mut builder = ReflectionRegistryBuilder::new();
        builder.register::<u32>(&types).register::<f32>(&types);
        let registry = builder.build();

        assert_eq!(registry.len(), 2);
        assert!(registry.get_of::<u32>(&types).unwrap().as_value().is_some());
        assert!(registry.get(other.type_id_of::<f32>().any_type_id()).is_none());

        let value = 2.0_f32;
        let foreign = AnyRef::new(&value, other.type_id_of());
        assert_eq!(
            registry.get_for(foreign).unwrap().type_id(),
            types.type_id_of::<f32>().any_type_id()
        );
    }
}
