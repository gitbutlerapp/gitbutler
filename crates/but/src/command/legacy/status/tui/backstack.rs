use std::{collections::VecDeque, ops::Deref};

#[derive(Debug, Default, Clone)]
pub(super) struct Backstack {
    stack: VecDeque<BackstackEntry>,
}

impl Backstack {
    pub(super) fn push_leave_normal_mode(&mut self) {
        self.push_front(BackstackEntry::LeaveNormalMode);
    }

    pub(super) fn remove_leave_normal_mode(&mut self) {
        self.remove(BackstackEntry::LeaveNormalMode);
    }

    pub(super) fn push_show_file_list(&mut self) {
        self.push_front(BackstackEntry::ShowFileList);
    }

    pub(super) fn push_open_details_view(&mut self, full_screen: bool) {
        self.push_front(if full_screen {
            BackstackEntry::OpenFullScreenDetailsView
        } else {
            BackstackEntry::OpenSplitDetailsView
        });
    }

    pub(super) fn remove_show_file_list(&mut self) {
        self.remove(BackstackEntry::ShowFileList);
    }

    pub(super) fn remove_open_details_view(&mut self) {
        self.remove(BackstackEntry::OpenSplitDetailsView);
        self.remove(BackstackEntry::OpenFullScreenDetailsView);
    }

    pub(super) fn push_mark(&mut self) {
        self.remove_mark();
        self.push_front(BackstackEntry::Mark);
    }

    pub(super) fn remove_mark(&mut self) {
        self.remove(BackstackEntry::Mark);
    }

    fn push_front(&mut self, entry: BackstackEntry) {
        self.stack.push_front(entry);
    }

    fn remove(&mut self, entry: BackstackEntry) {
        self.stack.retain(|e| *e != entry);
    }

    pub(super) fn pop(&mut self) -> Option<BackstackEntry> {
        self.stack.pop_front()
    }

    pub(super) fn retain<F>(&mut self, mut f: F)
    where
        F: FnMut(BackstackEntry) -> bool,
    {
        self.stack.retain(|entry| f(*entry));
    }

    #[expect(dead_code)]
    pub(super) fn clear(&mut self) {
        self.stack.clear();
    }

    #[cfg(test)]
    pub(super) fn iter(&self) -> impl Iterator<Item = &BackstackEntry> {
        self.stack.iter()
    }
}

#[derive(Debug, PartialEq, Copy, Clone)]
pub(super) enum BackstackEntry {
    LeaveNormalMode,
    ShowFileList,
    Mark,
    OpenSplitDetailsView,
    OpenFullScreenDetailsView,
}

/// Wrapper type that makes it hard to forget updating the backstack when updating the inner value.
#[derive(Debug, Default, Clone, Copy)]
pub(super) struct RememberToUpdateBackstack<T>(T);

impl<T> RememberToUpdateBackstack<T> {
    pub(super) fn new(value: T) -> Self {
        Self(value)
    }

    /// Get mutable access to the inner value together with the backstack.
    ///
    /// rustc will give "unused variable" warnings if we forget to use the backstack passed into
    /// the closure. This makes it less likely we'll forget to update the backstack.
    #[inline]
    pub(super) fn update<F, K>(&mut self, backstack: &mut Backstack, f: F) -> K
    where
        F: FnOnce(&mut Backstack, &mut T) -> K,
    {
        f(backstack, &mut self.0)
    }

    /// Get mutable access to the inner value while pushing [`Backstack::LeaveNormalMode`].
    #[inline]
    pub(super) fn update_and_push_leave_normal_mode<F, K>(
        &mut self,
        backstack: &mut Backstack,
        f: F,
    ) -> K
    where
        F: FnOnce(&mut T) -> K,
    {
        backstack.remove_leave_normal_mode();
        backstack.push_leave_normal_mode();
        f(&mut self.0)
    }

    /// Get a mutable reference to the inner value, without being reminded to update the backstack.
    ///
    /// This method should only be used if you're mutating a part of the state and not replacing it
    /// outright.
    #[inline]
    pub(super) fn get_mut_without_updating_backstack_and_i_promise_not_to_change_state(
        &mut self,
    ) -> &mut T {
        &mut self.0
    }
}

impl<T> Deref for RememberToUpdateBackstack<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
