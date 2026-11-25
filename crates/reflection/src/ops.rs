use aphyn_ffi::{FfiAny, FfiAnyMut, FfiAnyRef, FfiString, FfiVec};

use crate::{EnumVariantReflector, ReflectionRegistry, Reflector};

pub fn reflect_default(registry: &ReflectionRegistry, type_name: &str) -> Option<FfiAny> {
    registry.get(type_name)?.default_value()
}

// the variant with every field set to its type's default
pub fn reflect_construct_variant(registry: &ReflectionRegistry, variant: &EnumVariantReflector) -> Option<FfiAny> {
    let fields = variant
        .fields()
        .map(|f| reflect_default(registry, f.dst_type_name()))
        .collect::<Option<Vec<_>>>()?;

    variant.construct(fields.into())
}

// the type's clone factory, otherwise its default filled with clones of the reflected parts
pub fn reflect_clone(registry: &ReflectionRegistry, value: FfiAnyRef) -> Option<FfiAny> {
    let reflector = registry.get(value.type_info().short().name())?;

    if let Some(cloned) = reflector.clone_value(value) {
        return Some(cloned);
    }

    match &**reflector {
        Reflector::Value(_) => None,
        Reflector::Struct(r) => {
            let mut dst = reflector.default_value()?;

            for field in r.fields() {
                let cloned = reflect_clone(registry, field.get(value)?)?;
                field.get_mut(dst.as_any_mut())?.replace(cloned).ok()?;
            }

            Some(dst)
        }
        Reflector::Enum(r) => {
            let variant = r.active_variant(value)?;
            let fields = variant
                .fields()
                .map(|f| reflect_clone(registry, f.get(value)?))
                .collect::<Option<Vec<_>>>()?;

            variant.construct(fields.into())
        }
        Reflector::List(r) => {
            let mut dst = reflector.default_value()?;
            let len = r.len(value)?;

            if r.is_rigid() {
                if r.len(dst.as_any_ref())? != len {
                    return None;
                }
                for i in 0..len {
                    let cloned = reflect_clone(registry, r.get(value, i)?)?;
                    r.get_mut(dst.as_any_mut(), i)?.replace(cloned).ok()?;
                }
            } else {
                for i in 0..len {
                    let cloned = reflect_clone(registry, r.get(value, i)?)?;
                    let end = r.len(dst.as_any_ref())?;
                    r.insert(dst.as_any_mut(), end, cloned).ok()?;
                }
            }

            Some(dst)
        }
        Reflector::Map(r) => {
            let mut dst = reflector.default_value()?;

            for key in r.keys(value)? {
                let cloned_key = reflect_clone(registry, key)?;
                let cloned_value = reflect_clone(registry, r.get(value, key)?)?;

                if let Err((cloned_key, cloned_value)) = r.insert(dst.as_any_mut(), cloned_key, cloned_value) {
                    r.get_mut(dst.as_any_mut(), cloned_key.as_any_ref())?
                        .replace(cloned_value)
                        .ok()?;
                }
            }

            Some(dst)
        }
    }
}

//

#[repr(C)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ReflectPathSegment {
    // a struct field, or a field of the active enum variant
    Field(FfiString),
    // a list element, or the map entry at this position of `MapReflector::keys`
    Index(u64),
}

#[repr(C)]
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ReflectPath(pub FfiVec<ReflectPathSegment>);

impl ReflectPath {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn segments(&self) -> &[ReflectPathSegment] {
        self.0.as_slice()
    }

    pub fn join(&self, segment: ReflectPathSegment) -> Self {
        let mut path = self.clone();
        path.0.push(segment);
        path
    }

    pub fn parent(&self) -> Option<Self> {
        let mut path = self.clone();
        path.0.pop()?;
        Some(path)
    }

    // the inverse of `Display`: "" is the root, "/field/[3]" a field then an index
    pub fn parse(text: &str) -> Option<Self> {
        let mut path = Self::new();

        if text.is_empty() {
            return Some(path);
        }

        for segment in text.strip_prefix('/')?.split('/') {
            let segment = match segment.strip_prefix('[').and_then(|s| s.strip_suffix(']')) {
                Some(idx) => ReflectPathSegment::Index(idx.parse().ok()?),
                None if !segment.is_empty() => ReflectPathSegment::Field(segment.into()),
                None => return None,
            };
            path.0.push(segment);
        }

        Some(path)
    }
}

impl std::fmt::Display for ReflectPath {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for segment in self.segments() {
            match segment {
                ReflectPathSegment::Field(name) => write!(f, "/{name}")?,
                ReflectPathSegment::Index(idx) => write!(f, "/[{idx}]")?,
            }
        }
        Ok(())
    }
}

pub fn resolve<'a>(registry: &ReflectionRegistry, mut value: FfiAnyRef<'a>, path: &[ReflectPathSegment]) -> Option<FfiAnyRef<'a>> {
    for segment in path {
        let reflector = registry.get(value.type_info().short().name())?;

        value = match (&**reflector, segment) {
            (Reflector::Struct(r), ReflectPathSegment::Field(name)) => r.field(name)?.get(value)?,
            (Reflector::Enum(r), ReflectPathSegment::Field(name)) => r.active_variant(value)?.field(name)?.get(value)?,
            (Reflector::List(r), ReflectPathSegment::Index(idx)) => r.get(value, *idx as usize)?,
            (Reflector::Map(r), ReflectPathSegment::Index(idx)) => {
                let keys = r.keys(value)?;
                r.get(value, *keys.get(*idx)?)?
            }
            _ => return None,
        };
    }

    Some(value)
}

// map keys are cloned (see `reflect_clone`) to look an entry up mutably
pub fn resolve_mut<'a>(
    registry: &ReflectionRegistry,
    mut value: FfiAnyMut<'a>,
    path: &[ReflectPathSegment],
) -> Option<FfiAnyMut<'a>> {
    for segment in path {
        let reflector = registry.get(value.type_info().short().name())?;

        value = match (&**reflector, segment) {
            (Reflector::Struct(r), ReflectPathSegment::Field(name)) => r.field(name)?.get_mut(value)?,
            (Reflector::Enum(r), ReflectPathSegment::Field(name)) => {
                r.active_variant(value.as_any_ref())?.field(name)?.get_mut(value)?
            }
            (Reflector::List(r), ReflectPathSegment::Index(idx)) => r.get_mut(value, *idx as usize)?,
            (Reflector::Map(r), ReflectPathSegment::Index(idx)) => {
                let key = {
                    let keys = r.keys(value.as_any_ref())?;
                    reflect_clone(registry, *keys.get(*idx)?)?
                };
                r.get_mut(value, key.as_any_ref())?
            }
            _ => return None,
        };
    }

    Some(value)
}
