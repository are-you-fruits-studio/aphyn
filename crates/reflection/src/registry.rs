use aphyn_ffi::{FfiArc, FfiIndexMap, FfiString};

use crate::{ReflectField, Reflectable, Reflector};

#[repr(C)]
#[derive(Default)]
pub struct ReflectionRegistryBuilder {
    reflectors: FfiIndexMap<FfiString, Reflector>,
}

impl ReflectionRegistryBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register<T: Reflectable>(&mut self) -> &mut Self {
        if !self.reflectors.contains_key(std::any::type_name::<T>()) {
            self.register_reflector(T::reflector());
        }
        self
    }

    pub fn register_reflector(&mut self, reflector: Reflector) -> &mut Self {
        self.reflectors.entry(reflector.type_name().into()).or_insert(reflector);
        self
    }

    pub fn contains(&self, type_name: &str) -> bool {
        self.reflectors.contains_key(type_name)
    }

    pub fn build(self) -> ReflectionRegistry {
        ReflectionRegistry {
            reflectors: FfiArc::new(self.reflectors.into_iter().map(|(k, v)| (k, FfiArc::new(v))).collect()),
        }
    }
}

#[repr(C)]
#[derive(Clone)]
pub struct ReflectionRegistry {
    reflectors: FfiArc<FfiIndexMap<FfiString, FfiArc<Reflector>>>,
}

impl ReflectionRegistry {
    pub fn get(&self, type_name: &str) -> Option<&FfiArc<Reflector>> {
        self.reflectors.get(type_name)
    }

    pub fn get_of<T: 'static>(&self) -> Option<&FfiArc<Reflector>> {
        self.get(std::any::type_name::<T>())
    }

    pub fn dst_of(&self, field: &ReflectField) -> Option<&FfiArc<Reflector>> {
        self.get(field.dst_type_name())
    }

    pub fn iter(&self) -> impl Iterator<Item = &FfiArc<Reflector>> {
        self.reflectors.values()
    }

    pub fn len(&self) -> usize {
        self.reflectors.len() as usize
    }

    pub fn is_empty(&self) -> bool {
        self.reflectors.is_empty()
    }

    pub fn missing_dst_types(&self) -> Vec<&'static str> {
        let mut missing = Vec::new();
        let mut check = |type_name: &'static str| {
            if !self.reflectors.contains_key(type_name) && !missing.contains(&type_name) {
                missing.push(type_name);
            }
        };

        for reflector in self.iter() {
            match &**reflector {
                Reflector::Value(_) => {}
                Reflector::Struct(r) => r.fields().for_each(|f| check(f.dst_type_name())),
                Reflector::Enum(r) => r.variants().flat_map(|v| v.fields()).for_each(|f| check(f.dst_type_name())),
                Reflector::Collection(r) => check(r.element_type_name()),
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
        f.debug_list().entries(self.reflectors.keys()).finish()
    }
}
