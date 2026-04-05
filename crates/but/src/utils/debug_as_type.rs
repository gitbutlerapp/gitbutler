use std::ops::{Deref, DerefMut};

/// Transparent wrapper type that adds a blanket `Debug` implementation that uses the type name,
/// instead of the runtime value.
///
/// This is useful when including some non-`Debug` field in a type with `#[derive(Debug)]`.
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub(crate) struct DebugAsType<T>(pub(crate) T);

impl<T> std::fmt::Debug for DebugAsType<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(std::any::type_name::<T>())
    }
}

impl<T> From<T> for DebugAsType<T> {
    fn from(value: T) -> Self {
        Self(value)
    }
}

impl<T> Deref for DebugAsType<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T> DerefMut for DebugAsType<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
