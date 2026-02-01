use std::{cell, cell::RefCell, rc::Rc};

/// A utility to keep a value and initialize it on the fly, with the value being a cache that is mutable without official mutation.
///
/// This structure trades compile-time safety for being able to 'hide' that caches are actually changed as they allow accessing
/// cached data mutably on a *shared* borrow.
/// Caches must also infallibly initialize to ensure that applications always work even without a cache, or an empty cache. To facilitate
/// this, the consuming code will always get to work with a cache, and should choose to ignore errors where possible - in the worst case,
/// it can recalculate the cached data.
///
/// Otherwise, equivalent to [`OnDemand`](crate::OnDemand)
pub struct OnDemandCache<T> {
    init: Rc<dyn Fn() -> T + 'static>,
    value: cell::RefCell<Option<T>>,
}

impl<T> Clone for OnDemandCache<T>
where
    T: Clone,
{
    fn clone(&self) -> Self {
        Self {
            init: self.init.clone(),
            value: self.value.clone(),
        }
    }
}

/// Lifecycle
impl<T> OnDemandCache<T> {
    /// Create a new instance that can instantiate its value via `init` when needed.
    pub fn new(init: impl Fn() -> T + 'static) -> Self {
        OnDemandCache {
            init: Rc::new(init),
            value: RefCell::new(None),
        }
    }
}

/// Access
impl<T> OnDemandCache<T> {
    /// Get a shared reference to the cached value or initialise it.
    pub fn get_cache(&self) -> Result<cell::Ref<'_, T>, BorrowError> {
        if let Ok(cached) = cell::Ref::filter_map(self.value.try_borrow()?, |opt| opt.as_ref()) {
            return Ok(cached);
        }
        {
            let mut value = self.value.try_borrow_mut()?;
            *value = Some((self.init)());
        }
        Ok(
            cell::Ref::filter_map(self.value.borrow(), |opt| opt.as_ref())
                .unwrap_or_else(|_| unreachable!("just set the value")),
        )
    }

    /// Get an exclusive references to the cached value or fallibly initialise it.
    pub fn get_cache_mut(&self) -> Result<cell::RefMut<'_, T>, BorrowError> {
        if let Ok(cached) =
            cell::RefMut::filter_map(self.value.try_borrow_mut()?, |opt| opt.as_mut())
        {
            return Ok(cached);
        }
        {
            let mut value = self.value.try_borrow_mut()?;
            *value = Some((self.init)());
        }
        Ok(
            cell::RefMut::filter_map(self.value.borrow_mut(), |opt| opt.as_mut())
                .unwrap_or_else(|_| unreachable!("just set the value")),
        )
    }
}

mod error {
    use std::{cell, fmt::Formatter};

    pub enum BorrowError {
        Shared(cell::BorrowError),
        Exclusive(cell::BorrowMutError),
    }

    impl From<cell::BorrowError> for BorrowError {
        fn from(value: cell::BorrowError) -> Self {
            Self::Shared(value)
        }
    }

    impl From<cell::BorrowMutError> for BorrowError {
        fn from(value: cell::BorrowMutError) -> Self {
            Self::Exclusive(value)
        }
    }

    impl std::fmt::Display for BorrowError {
        fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
            match self {
                BorrowError::Shared(e) => std::fmt::Display::fmt(&e, f),
                BorrowError::Exclusive(e) => std::fmt::Display::fmt(&e, f),
            }
        }
    }

    impl std::fmt::Debug for BorrowError {
        fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
            match self {
                BorrowError::Shared(e) => std::fmt::Debug::fmt(&e, f),
                BorrowError::Exclusive(e) => std::fmt::Debug::fmt(&e, f),
            }
        }
    }

    impl std::error::Error for BorrowError {}
}
use error::BorrowError;

#[cfg(test)]
mod tests {
    use crate::OnDemand;

    #[test]
    fn on_demand_journey() -> anyhow::Result<()> {
        let mut v = OnDemand::new(|| Ok(42usize));
        let vr = *v.get()?;
        assert_eq!(vr, 42);
        assert_eq!(*v.get()?, 42, "double read-only borrow is fine");

        {
            let mut vr = v.get_mut()?;
            assert_eq!(*vr, 42);
            *vr = 52;
            assert_eq!(*vr, 52);
        }

        assert_eq!(*v.get_mut()?, 52);
        assert_eq!(*v.get()?, 52);
        Ok(())
    }
}
