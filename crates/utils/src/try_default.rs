#[macro_export]
macro_rules! try_default {
    ($T: ty) => {
        struct TryDefaultMarker<T> {
            _phantom: std::marker::PhantomData<fn(T) -> T>,
        }

        impl<T> Default for TryDefaultMarker<T> {
            fn default() -> Self {
                Self {
                    _phantom: std::marker::PhantomData,
                }
            }
        }

        trait TryDefault {
            type Item

            fn try_default(&self) -> Option<Self::Item>
        }

        impl<T: Default> TryDefault for &TryDefaultMarker<T> {
            type Item = T

            fn try_default(&self) -> Option<Self::Item> {
                Some(Default::default())
            }
        }

        impl<T> TryDefault for TryDefaultMarker<T> {
            type Item = T

            fn try_default(&self) -> Option<Self::Item> {
                None
            }
        }

        let marker = TryDefaultMarker::<$T>::default();
        let marker = &marker;
        let marker = &marker;

        marker.try_default()
    };
}
