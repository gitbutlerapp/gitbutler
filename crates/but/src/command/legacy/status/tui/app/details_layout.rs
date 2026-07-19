use crate::command::legacy::status::tui::{
    DETAILS_MAX_SIZE_PERCENTAGE, DETAILS_MIN_SIZE_PERCENTAGE, DetailsLayoutMessage, Message,
    app::{App, normal_mode::NormalMode},
    details::DetailsMessage,
    mode::{DetailsMode, DetailsReturnMode, Mode},
};

impl App {
    pub fn handle_unfocus_details(&mut self, messages: &mut Vec<Message>) {
        if let Mode::Details(DetailsMode { full_screen, .. }) = self
            .mode
            .get_mut_and_i_promise_not_to_switch_to_a_different_state()
        {
            if *full_screen {
                return;
            }

            self.restore_mode_before_details(messages);

            if !self.maybe_move_cursor_into_file_list() {
                // the line that we were on when we opened the detail view must have been
                // selectable since otherwise the cursor couldn't have been on it
            }

            return;
        }

        self.unfocus_details_regardless_if_we_are_full_screen_or_not(messages);
    }

    pub fn unfocus_details_regardless_if_we_are_full_screen_or_not(
        &mut self,
        messages: &mut Vec<Message>,
    ) {
        self.mode.update(&mut self.backstack, |backstack, mode| {
            if let Mode::Details(details_mode) = mode
                && !details_mode.return_mode.marks().is_empty()
            {
                backstack.remove_mark();
            }
            *mode = Mode::Normal(Default::default());
            backstack.remove_leave_normal_mode();
        });

        messages.push(Message::Details(DetailsMessage::Deselect));
    }

    pub fn restore_mode_before_details(&mut self, messages: &mut Vec<Message>) -> bool {
        self.mode.update(&mut self.backstack, |backstack, mode| {
            let previous_mode = std::mem::replace(mode, Mode::Normal(NormalMode::default()));
            let Mode::Details(details_mode) = previous_mode else {
                *mode = previous_mode;
                return false;
            };

            backstack.remove_leave_normal_mode();
            if details_mode.full_screen {
                backstack.remove_open_details_view();
                messages.push(Message::DetailsLayout(
                    DetailsLayoutMessage::ToggleVisibility,
                ));
            } else {
                messages.push(Message::Details(DetailsMessage::Deselect));
            }

            *mode = match details_mode.return_mode {
                DetailsReturnMode::Normal(normal_mode) => Mode::Normal(normal_mode),
                DetailsReturnMode::PickChanges(pick_uncommitted_mode) => {
                    Mode::PickChanges(pick_uncommitted_mode)
                }
            };
            true
        })
    }

    pub fn handle_focus_details(&mut self, full_screen: bool, messages: &mut Vec<Message>) {
        if !full_screen {
            match &*self.mode {
                Mode::Details(DetailsMode {
                    full_screen: false, ..
                }) => return,
                Mode::Details(DetailsMode {
                    full_screen: true, ..
                }) => {
                    messages.push(Message::DetailsLayout(DetailsLayoutMessage::SwitchToSplit));
                    return;
                }
                Mode::Normal(..)
                | Mode::PickChanges(..)
                | Mode::Rub(..)
                | Mode::InlineReword(..)
                | Mode::Command(..)
                | Mode::Commit(..)
                | Mode::Move(..)
                | Mode::MoveStack(..)
                | Mode::Jump(..)
                | Mode::Stack(..) => {}
            }
        }

        if full_screen
            && self.is_details_visible
            && matches!(
                &*self.mode,
                Mode::Details(DetailsMode {
                    full_screen: false,
                    ..
                })
            )
        {
            messages.push(Message::Details(DetailsMessage::SelectFirstSection));
            self.mode.update(&mut self.backstack, |backstack, mode| {
                // This is only a layout change, so the existing undo order must be preserved.
                let _ = backstack;
                let Mode::Details(details_mode) = mode else {
                    unreachable!("details mode was checked above")
                };
                details_mode.full_screen = true;
            });
            return;
        }

        if self.is_details_visible {
            messages.push(Message::Details(DetailsMessage::SelectFirstSection));
        } else {
            messages.push(Message::DetailsLayout(
                DetailsLayoutMessage::ToggleVisibility,
            ));
            messages.push(Message::Details(DetailsMessage::SelectFirstSection));

            self.backstack.push_open_details_view(full_screen);
        }

        self.mode
            .update_and_push_leave_normal_mode(&mut self.backstack, |mode| {
                let previous_mode = std::mem::replace(mode, Mode::Normal(NormalMode::default()));
                let return_mode = match previous_mode {
                    Mode::PickChanges(pick_uncommitted_mode) => {
                        DetailsReturnMode::PickChanges(pick_uncommitted_mode)
                    }
                    Mode::Details(details_mode) => details_mode.return_mode,
                    Mode::Normal(normal_mode) => DetailsReturnMode::Normal(normal_mode),
                    Mode::Rub(..)
                    | Mode::InlineReword(..)
                    | Mode::Command(..)
                    | Mode::Commit(..)
                    | Mode::Move(..)
                    | Mode::MoveStack(..)
                    | Mode::Jump(..)
                    | Mode::Stack(..) => DetailsReturnMode::Normal(NormalMode::default()),
                };
                *mode = Mode::Details(DetailsMode {
                    full_screen,
                    return_mode,
                });
            });
    }

    pub fn handle_switch_details_to_split(&mut self) {
        self.mode.update(&mut self.backstack, |backstack, mode| {
            let Mode::Details(DetailsMode { full_screen, .. }) = mode else {
                return;
            };
            if !*full_screen {
                return;
            }

            *full_screen = false;
            backstack.switch_full_screen_details_to_split();
        });
    }

    pub fn handle_toggle_details_full_screen(&mut self, messages: &mut Vec<Message>) {
        match self
            .mode
            .get_mut_and_i_promise_not_to_switch_to_a_different_state()
        {
            Mode::Normal(..) | Mode::PickChanges(..) => {
                messages.push(Message::DetailsLayout(DetailsLayoutMessage::Focus {
                    full_screen: true,
                }));
            }
            Mode::Details(DetailsMode { full_screen, .. }) => {
                if *full_screen {
                    self.restore_mode_before_details(messages);
                } else {
                    messages.push(Message::DetailsLayout(DetailsLayoutMessage::Focus {
                        full_screen: true,
                    }));
                }
            }
            Mode::Rub(..)
            | Mode::InlineReword(..)
            | Mode::Command(..)
            | Mode::Commit(..)
            | Mode::Stack(..)
            | Mode::MoveStack(..)
            | Mode::Jump(..)
            | Mode::Move(..) => {}
        }
    }

    pub fn handle_toggle_details_visibility(&mut self, messages: &mut Vec<Message>) {
        self.is_details_visible = !self.is_details_visible;

        if self.is_details_visible {
            if matches!(&*self.mode, Mode::Normal(..)) {
                self.backstack.push_open_details_view(false);
            }
        } else {
            self.details.on_hidden();
            self.backstack.remove_open_details_view();
            if matches!(&*self.mode, Mode::Details(..)) {
                messages.push(Message::UnfocusDetails);
            }
        }
    }

    pub fn handle_dismiss_details(&mut self, messages: &mut Vec<Message>) {
        if let Mode::Details(details_mode) = &*self.mode
            && details_mode.full_screen
        {
            messages.push(Message::DetailsLayout(
                DetailsLayoutMessage::ToggleFullScreen,
            ));
        } else {
            messages.push(Message::DetailsLayout(
                DetailsLayoutMessage::ToggleVisibility,
            ));
        }
    }

    pub fn update_status_width_percentage(&mut self, new: u16) {
        if !self.is_details_visible {
            return;
        }

        self.status_width_percentage = new.clamp(
            100 - DETAILS_MAX_SIZE_PERCENTAGE,
            100 - DETAILS_MIN_SIZE_PERCENTAGE,
        );
    }
}
