#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct DefaultTrue(bool);

impl core::fmt::Debug for DefaultTrue {
    #[inline]
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        <bool as core::fmt::Debug>::fmt(&self.0, f)
    }
}

impl core::fmt::Display for DefaultTrue {
    #[inline]
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        <bool as core::fmt::Display>::fmt(&self.0, f)
    }
}

impl Default for DefaultTrue {
    #[inline]
    fn default() -> Self {
        DefaultTrue(true)
    }
}

impl From<DefaultTrue> for bool {
    #[inline]
    fn from(default_true: DefaultTrue) -> Self {
        default_true.0
    }
}

impl From<bool> for DefaultTrue {
    #[inline]
    fn from(boolean: bool) -> Self {
        DefaultTrue(boolean)
    }
}

impl serde::Serialize for DefaultTrue {
    #[inline]
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_bool(self.0)
    }
}

impl<'de> serde::Deserialize<'de> for DefaultTrue {
    #[inline]
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Ok(DefaultTrue(bool::deserialize(deserializer)?))
    }
}

impl core::ops::Deref for DefaultTrue {
    type Target = bool;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl core::ops::DerefMut for DefaultTrue {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl PartialEq<bool> for DefaultTrue {
    #[inline]
    fn eq(&self, other: &bool) -> bool {
        self.0 == *other
    }
}

impl PartialEq<DefaultTrue> for bool {
    #[inline]
    fn eq(&self, other: &DefaultTrue) -> bool {
        *self == other.0
    }
}

impl core::ops::Not for DefaultTrue {
    type Output = bool;

    #[inline]
    fn not(self) -> Self::Output {
        !self.0
    }
}

#[test]
#[expect(clippy::bool_assert_comparison)]
fn default_true() {
    let default_true = DefaultTrue::default();
    assert!(default_true);
    assert_eq!(default_true, true);
    assert_eq!(!default_true, false);
    assert!(!!default_true);

    if !(*default_true) {
        unreachable!("default_true is false")
    }

    let mut default_true = DefaultTrue::default();
    *default_true = false;
    assert!(!default_true);
}
