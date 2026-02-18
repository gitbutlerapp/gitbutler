use crate::{engine::CommandBuffer, input::Key, prompts::EventOutcome, style::ConfirmationStyle};

use super::Prompt;

/// A prompt that expects a "yes or no" answer.
/// You can press 'y' or 'n' for positive or negative result.
/// Pressing 'Enter' without any previous input will trigger the default ansewer, which is
/// configurable
///
/// ```rust
/// use cli_prompts::{
///     prompts::{Confirmation, AbortReason},
///     style::{ConfirmationStyle, Formatting},
///     DisplayPrompt,
/// };
///
/// fn main() {
///     let prompt = Confirmation::new("Are you sure you want to delete this file?")
///                     .default_positive(false)
///                     .style(ConfirmationStyle::default());
///     let answer : Result<bool, AbortReason> = prompt.display();
///     match answer {
///         Ok(is_positive) => println!("The answer is {}", is_positive),
///         Err(abort_reason) => println!("The prompt was aborted because of {:?}", abort_reason),
///     }
/// }
/// ```
pub struct Confirmation {
    label: String,
    default_positive: bool,
    is_submitted: bool,
    selected_option: Option<bool>,
    style: ConfirmationStyle,
}

impl Confirmation {

    /// Constructs a new prompt with a given label
    pub fn new<S: Into<String>>(label: S) -> Self {
        Confirmation {
            label: label.into(),
            default_positive: true,
            is_submitted: false,
            selected_option: None,
            style: ConfirmationStyle::default(),
        }
    }

    /// Sets whether the default value is positive or negative
    pub fn default_positive(mut self, default_positive: bool) -> Self {
        self.default_positive = default_positive;
        self
    }

    /// Sets the style of the prompt
    pub fn style(mut self, s: ConfirmationStyle) -> Self {
        self.style = s;
        self
    }
}

impl Prompt<bool> for Confirmation {
    fn draw(&self, commands: &mut impl CommandBuffer) {
        self.style.label_style.print(
            format!(
                "{} [{y}/{n}]",
                self.label,
                y = if self.default_positive { 'Y' } else { 'y' },
                n = if !self.default_positive { 'N' } else { 'n' },
            ),
            commands,
        );

        let result: String = if let Some(is_positive) = self.selected_option.as_ref() {
            if *is_positive {
                "Yes".into()
            } else {
                "No".into()
            }
        } else {
            String::new()
        };

        let formatting = if self.is_submitted {
            &self.style.submitted_formatting
        } else {
            &self.style.input_formatting
        };

        formatting.print(result, commands);
    }

    fn on_key_pressed(&mut self, key: Key) -> EventOutcome<bool> {
        match key {
            Key::Enter => {
                self.is_submitted = true;
                if let Some(is_positive) = self.selected_option.as_ref() {
                    EventOutcome::Done(*is_positive)
                } else {
                    self.selected_option = Some(self.default_positive);
                    EventOutcome::Done(self.default_positive)
                }
            }
            Key::Char(c) if self.selected_option.is_none() => match c {
                'y' | 'Y' => {
                    self.selected_option = Some(true);
                    EventOutcome::Continue
                }
                'n' | 'N' => {
                    self.selected_option = Some(false);
                    EventOutcome::Continue
                }
                _ => EventOutcome::Continue,
            },
            Key::Backspace => {
                self.selected_option = None;
                EventOutcome::Continue
            }
            Key::Esc => EventOutcome::Abort(super::AbortReason::Interrupt),
            _ => EventOutcome::Continue,
        }
    }
}
