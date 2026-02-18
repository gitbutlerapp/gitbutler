use super::Prompt;
use crate::{
    engine::CommandBuffer,
    input::Key,
    prompts::{AbortReason, EventOutcome},
    style::InputStyle,
};

/// This is a normal text input prompt with the following features:
/// - Custom label
/// - Validation of the input with error reporting
/// - Transformation of the text input to arbitrary Rust type
/// - Optional default value
/// - Optional help message
/// - Customizable colors and formatting
///
/// ```rust
/// use cli_prompts::{
///     prompts::{Input, AbortReason::{self, Error}},
///     style::{InputStyle, Formatting},
///     DisplayPrompt
/// };
///
/// fn validate_and_transform(input: &str) -> Result<u16, String> {
///     input
///       .parse::<u16>()
///       .map_err(|e| "Provided input is not a valid time of the day".into())
///       .and_then(|n| if n <= 24 { Ok(n) } else { Err("Provided input is not a valid time of the day".into()) })
/// }
///
/// fn main() {
///     let input = Input::new("At what time do you usually eat lunch?", validate_and_transform)
///         .default_value("12")
///         .help_message("Enter an hour of the day as a number from 0 to 24")
///         .style(InputStyle::default()
///                   .default_value_formatting(Formatting::default().bold())
///          );
///
///     let lunch_time : Result<u16, AbortReason> = input.display();
///     match lunch_time {
///         Ok(time) => println!("You eat lunch at {} o'clock", time),
///         Err(abort_reason) => match abort_reason {
///             Interrupt => println!("The prompt was interrupted by pressing the ESC key"),
///             Error(err) => println!("I/O error has occured: {:?}", err),
///         }
///     }
///     
/// }
/// ```
pub struct Input<F> {
    label: String,
    input: String,
    help_message: Option<String>,
    is_first_input: bool,
    is_submitted: bool,
    error: Option<String>,
    validation: F,
    style: InputStyle,
}

impl<F, T> Input<F>
where
    F: Fn(&str) -> Result<T, String>,
{
    /// Constructs an input prompt with a given label and a validation function
    /// The function serves both as validator and transformer: it should return `Ok`
    /// of the arbitrary type `T` if validation passed and `Err(String)` if it failed.
    /// The containing String will be displayed as an error message and the prompt will continue
    /// until this function returns Ok
    pub fn new(label: impl Into<String>, validation: F) -> Self {
        Self {
            label: label.into(),
            input: String::new(),
            help_message: None,
            is_first_input: true,
            is_submitted: false,
            error: None,
            validation,
            style: InputStyle::default(),
        }
    }

    /// Sets a help message which will be displayed after the input string
    /// until the prompt is completed
    pub fn help_message<S: Into<String>>(mut self, message: S) -> Self {
        self.help_message = Some(message.into());
        self
    }

    /// Sets the default value for the prompt. It is cleared once any key
    /// is pressed other than Enter.
    pub fn default_value<S: Into<String>>(mut self, val: S) -> Self {
        self.input = val.into();
        self
    }

    /// Sets the style for the prompt
    pub fn style(mut self, style: InputStyle) -> Self {
        self.style = style;
        self
    }
}

impl<T, F> Prompt<T> for Input<F>
where
    F: Fn(&str) -> Result<T, String>,
{
    fn draw(&self, commands: &mut impl CommandBuffer) {
        self.style.label_style.print(&self.label, commands);

        if let Some(error) = self.error.as_ref() {
            self.style
                .error_formatting
                .print(format!("[{}]", error), commands);
        } else if self.is_submitted {
            self.style.submitted_formatting.print(&self.input, commands);
        } else if self.is_first_input && self.input.len() > 0 {
            self.style
                .default_value_formatting
                .print(format!("[{}]", self.input), commands);
        } else if !self.is_first_input {
            self.style.input_formatting.print(&self.input, commands);
        }

        if let Some(help_message) = self.help_message.as_ref() {
            self.style
                .help_message_formatting
                .print(format!("[{}]", help_message), commands);
        }
    }

    fn on_key_pressed(&mut self, key: Key) -> EventOutcome<T> {
        let is_first_input = self.is_first_input;
        self.is_first_input = false;
        match key {
            Key::Char(c) => {
                if is_first_input {
                    self.input.clear();
                }
                self.error = None;
                self.input.push(c);
                EventOutcome::Continue
            }
            Key::Backspace => {
                if is_first_input {
                    self.input.clear();
                }
                self.error = None;
                self.input.pop();
                EventOutcome::Continue
            }
            Key::Enter => {
                self.error = (self.validation)(&self.input).err();
                match self.error {
                    Some(_) => {
                        self.input.clear();
                        EventOutcome::Continue
                    }
                    None => {
                        self.is_submitted = true;
                        EventOutcome::Done((self.validation)(&self.input).unwrap())
                    }
                }
            }
            Key::Esc => EventOutcome::Abort(AbortReason::Interrupt),
            _ => EventOutcome::Continue,
        }
    }
}
