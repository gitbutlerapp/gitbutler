use std::{cell, cell::RefCell, rc::Rc};

/// A utility to keep a value and initialize it on the fly, with the value being a cache that is mutable without official mutation.
///
/// This structure trades compile-time safety for being able to 'hide' that caches are actually changed as they allow accessing
/// cached data mutably on a *shared* borrow.
/// Caches *may* also infallibly initialize to ensure that applications always work even without a cache, or an empty cache. To facilitate
/// this, the consuming code will always get to work with a cache, and should choose to ignore errors where possible - in the worst case,
/// it can recalculate the cached data. Use [`OnDemandCache::new()`] for that.
///
/// If only interior mutability is needed, without guaranteed cache creation, use [`OnDemandCache::new_fallible()`].
///
/// Otherwise, equivalent to [`OnDemand`](crate::OnDemand)
pub struct OnDemandCache<T> {
    init: Rc<dyn Fn() -> anyhow::Result<T> + 'static>,
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
        Self::new_fallible(move || Ok(init()))
    }

    /// Create a new instance that can instantiate its value via `init` when needed,
    /// allowing initialization to fail.
    pub fn new_fallible(init: impl Fn() -> anyhow::Result<T> + 'static) -> Self {
        OnDemandCache {
            init: Rc::new(init),
            value: RefCell::new(None),
        }
    }
}

/// Access
impl<T> OnDemandCache<T> {
    /// Get a shared reference to the cached value or initialise it.
    pub fn get_cache(&self) -> anyhow::Result<cell::Ref<'_, T>> {
        if let Ok(cached) = cell::Ref::filter_map(self.value.try_borrow()?, |opt| opt.as_ref()) {
            return Ok(cached);
        }
        {
            let mut value = self.value.try_borrow_mut()?;
            *value = Some((self.init)()?);
        }
        Ok(
            cell::Ref::filter_map(self.value.borrow(), |opt| opt.as_ref())
                .unwrap_or_else(|_| unreachable!("just set the value")),
        )
    }

    /// Get an exclusive references to the cached value or fallibly initialise it.
    pub fn get_cache_mut(&self) -> anyhow::Result<cell::RefMut<'_, T>> {
        if let Ok(cached) =
            cell::RefMut::filter_map(self.value.try_borrow_mut()?, |opt| opt.as_mut())
        {
            return Ok(cached);
        }
        {
            let mut value = self.value.try_borrow_mut()?;
            *value = Some((self.init)()?);
        }
        Ok(
            cell::RefMut::filter_map(self.value.borrow_mut(), |opt| opt.as_mut())
                .unwrap_or_else(|_| unreachable!("just set the value")),
        )
    }
}

#[cfg(test)]
mod tests {
    use crate::OnDemandCache;

    #[test]
    fn on_demand_cache_journey() -> anyhow::Result<()> {
        let v = OnDemandCache::new(|| 42usize);
        assert_eq!(*v.get_cache()?, 42);
        assert_eq!(*v.get_cache()?, 42, "double read-only borrow is fine");

        {
            let mut vr = v.get_cache_mut()?;
            assert_eq!(*vr, 42);
            *vr = 52;
            assert_eq!(*vr, 52);
        }

        assert_eq!(*v.get_cache_mut()?, 52);
        assert_eq!(*v.get_cache()?, 52);
        Ok(())
    }
}
