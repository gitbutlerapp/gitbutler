use std::{collections::VecDeque, ops::Deref};

#[derive(Default, Clone)]
pub struct Backstack {
    stack: VecDeque<BackstackEntry>,
}

impl std::fmt::Debug for Backstack {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Self { stack } = self;
        stack.fmt(f)
    }
}

impl Backstack {
    pub fn push_leave_normal_mode(&mut self) {
        self.push_front(BackstackEntry::LeaveNormalMode);
    }

    pub fn push_leave_command_mode(&mut self) {
        self.push_front(BackstackEntry::LeaveCommandMode);
    }

    pub fn remove_leave_normal_mode(&mut self) {
        self.remove(BackstackEntry::LeaveNormalMode);
    }

    pub fn push_show_file_list(&mut self) {
        self.push_front(BackstackEntry::ShowFileList);
    }

    pub fn push_open_details_view(&mut self, full_screen: bool) {
        self.push_front(if full_screen {
            BackstackEntry::OpenFullScreenDetailsView
        } else {
            BackstackEntry::OpenSplitDetailsView
        });
    }

    pub fn remove_show_file_list(&mut self) {
        self.remove(BackstackEntry::ShowFileList);
    }

    pub fn remove_open_details_view(&mut self) {
        self.remove(BackstackEntry::OpenSplitDetailsView);
        self.remove(BackstackEntry::OpenFullScreenDetailsView);
    }

    pub fn switch_full_screen_details_to_split(&mut self) {
        for entry in &mut self.stack {
            if *entry == BackstackEntry::OpenFullScreenDetailsView {
                *entry = BackstackEntry::OpenSplitDetailsView;
            }
        }
    }

    pub fn push_mark(&mut self) {
        self.remove_mark();
        self.push_front(BackstackEntry::Mark);
    }

    pub fn remove_mark(&mut self) {
        self.remove(BackstackEntry::Mark);
    }

    fn push_front(&mut self, entry: BackstackEntry) {
        self.stack.push_front(entry);
    }

    fn remove(&mut self, entry: BackstackEntry) {
        self.stack.retain(|e| *e != entry);
    }

    pub fn pop(&mut self) -> Option<BackstackEntry> {
        self.stack.pop_front()
    }

    pub fn retain<F>(&mut self, mut f: F)
    where
        F: FnMut(BackstackEntry) -> bool,
    {
        self.stack.retain(|entry| f(*entry));
    }

    #[expect(dead_code)]
    pub fn clear(&mut self) {
        self.stack.clear();
    }

    #[cfg(test)]
    pub fn iter(&self) -> impl Iterator<Item = &BackstackEntry> {
        self.stack.iter()
    }
}

#[derive(Debug, PartialEq, Copy, Clone)]
pub enum BackstackEntry {
    LeaveNormalMode,
    LeaveCommandMode,
    ShowFileList,
    Mark,
    OpenSplitDetailsView,
    OpenFullScreenDetailsView,
}

/// Wrapper type that makes it hard to forget updating the backstack when updating the inner value.
#[derive(Debug, Default, Clone, Copy)]
pub struct RememberToUpdateBackstack<T>(T);

impl<T> RememberToUpdateBackstack<T> {
    pub fn new(value: T) -> Self {
        Self(value)
    }

    /// Get mutable access to the inner value together with the backstack.
    ///
    /// rustc will give "unused variable" warnings if we forget to use the backstack passed into
    /// the closure. This makes it less likely we'll forget to update the backstack.
    #[inline]
    pub fn update<F, K>(&mut self, backstack: &mut Backstack, f: F) -> K
    where
        F: FnOnce(&mut Backstack, &mut T) -> K,
    {
        f(backstack, &mut self.0)
    }

    /// Get mutable access to the inner value while pushing [`Backstack::LeaveNormalMode`].
    #[inline]
    pub fn update_and_push_leave_normal_mode<F, K>(&mut self, backstack: &mut Backstack, f: F) -> K
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
    pub fn get_mut_and_i_promise_not_to_switch_to_a_different_state(&mut self) -> &mut T {
        &mut self.0
    }
}

impl<T> Deref for RememberToUpdateBackstack<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
