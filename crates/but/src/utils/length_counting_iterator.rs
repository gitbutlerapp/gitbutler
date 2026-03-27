/// An iterator adapter that counts the number of elemented yielded by another iterator.
pub(crate) struct LengthCountingIterator<'a, I> {
    inner: I,
    len: &'a mut usize,
}

impl<'a, I> LengthCountingIterator<'a, I> {
    pub(crate) fn new(iter: I, len: &'a mut usize) -> Self {
        Self { inner: iter, len }
    }
}

impl<I> Iterator for LengthCountingIterator<'_, I>
where
    I: Iterator,
{
    type Item = I::Item;

    fn next(&mut self) -> Option<Self::Item> {
        let item = self.inner.next();
        if item.is_some() {
            *self.len = self.len.saturating_add(1);
        }
        item
    }
}

#[cfg(test)]
mod tests {
    #[allow(unused_imports)]
    use super::*;

    #[test]
    fn basic() {
        let mut len = 0;
        let iter = (0..10).filter(|n| n % 2 == 0);
        let iter = LengthCountingIterator::new(iter, &mut len);

        assert_eq!(iter.collect::<Vec<_>>(), Vec::from([0, 2, 4, 6, 8]));
        assert_eq!(len, 5);
    }
}
