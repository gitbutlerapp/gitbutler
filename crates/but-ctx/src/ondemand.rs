use std::cell;
use std::cell::RefCell;
use std::rc::Rc;

/// A utility to keep a cached value and initialize it on the fly.
///
/// Note that despite interior mutability, the structure is made to *not bypass* Rust's borrow-checker.
pub struct OnDemand<T> {
    init: Rc<dyn Fn() -> anyhow::Result<T> + 'static>,
    value: cell::RefCell<Option<T>>,
}

impl<T> Clone for OnDemand<T>
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

impl<T> OnDemand<T> {
    /// Create a new instance that can instantiate its value via `init` when needed.
    pub fn new(init: impl Fn() -> anyhow::Result<T> + 'static) -> Self {
        OnDemand {
            init: Rc::new(init),
            value: RefCell::new(None),
        }
    }

    /// Get a shared references to the cached value or fallibly initialise it.
    pub fn get(&self) -> anyhow::Result<cell::Ref<'_, T>> {
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
    pub fn get_mut(&mut self) -> anyhow::Result<cell::RefMut<'_, T>> {
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

    /// Return the existing value if there is one.
    pub fn get_opt(&self) -> cell::Ref<'_, Option<T>> {
        self.value.borrow()
    }

    /// Take the existing value out of the cache if there is one.
    ///
    /// On next access, the cache will be re-initialised.
    pub fn take(&mut self) -> Option<T> {
        self.value.borrow_mut().take()
    }

    /// Assign `value` and return a reference to it, dropping the previous cached value if it existed.
    // TODO: make this private and replace it with `Context` constructors or `with_*` post-construction modifiers.
    pub fn assign(&mut self, value: T) -> cell::RefMut<'_, T> {
        self.value = RefCell::new(Some(value));
        cell::RefMut::filter_map(self.value.borrow_mut(), |opt| opt.as_mut())
            .unwrap_or_else(|_| unreachable!("just set with a new value"))
    }
}
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
            {
                let mut vr = v.get_mut()?;
                assert_eq!(*vr, 42);
                *vr = 52;
                assert_eq!(*vr, 52);
            }
        }

        assert_eq!(*v.get_mut()?, 52);

        assert_eq!(*v.get()?, 52);

        {
            let vr = v.assign(100);
            assert_eq!(*vr, 100);
        }

        let mut v2 = v.clone();
        v2.assign(200);
        assert_eq!(*v2.get()?, 200);

        Ok(())
    }
}
