use std::ops::{Deref, DerefMut};

#[derive(Clone)]
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
