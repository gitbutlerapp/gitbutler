use crate::{
    engine::CommandBuffer,
    input::Key,
    prompts::{options::Options, AbortReason, EventOutcome, Prompt},
    style::MultiselectionStyle,
};

use super::multioption_prompt::MultiOptionPrompt;

const DEFAUTL_MAX_OPTIONS: u16 = 5;
const DEFAULT_HELP_MESSAGE: &str = "Space to select, enter to submit";

/// Prompt that allows to select multiple options from the given list.
/// Supports filtering and moving the selection with arrow keys.
///
/// ```rust
/// use cli_prompts::{
///     prompts::{Multiselect, AbortReason},
///     DisplayPrompt,
/// };
///
/// fn main() {
///     let files = [
///         "hello.txt",
///         "image.jpg",
///         "music.mp3",
///         "document.pdf"
///     ];
///
///     let prompt = Multiselect::new("Select files to copy", files.into_iter())
///                     .dont_display_help_message()
///                     .max_displayed_options(3);
///     let selection : Result<Vec<&str>, AbortReason> = prompt.display();
///     match selection {
///         Ok(selected_files) => {
///             for file in selected_files {
///                 // Copy the file
///             }
///         },
///         Err(abort_reason) => println!("Prompt is aborted because of {:?}", abort_reason),
///     }
/// }
/// ```
pub struct Multiselect<T> {
    label: String,
    options: Options<T>,
    selected_options: Vec<usize>,
    help_message: Option<String>,
    max_displayed_options: u16,
    currently_selected_index: usize,
    is_submitted: bool,
    filter: String,
    style: MultiselectionStyle,
}

impl<T> Multiselect<T>
where
    T: Into<String> + Clone,
{

    /// Create new prompt with the given label and the iterator over a type that is convertable to
    /// `String`
    pub fn new<S, I>(label: S, options: I) -> Self
    where
        S: Into<String>,
        I: Iterator<Item = T>,
    {
        let options = Options::from_iter(options);
        Self::new_internal(label.into(), options)
    }
}

impl<T> Multiselect<T> {
    
    /// Create new prompt with the given label and a transformation function that will convert the
    /// iterator items to strings
    pub fn new_transformed<S, I, F>(label: S, options: I, transformation: F) -> Self
    where
        S: Into<String>,
        I: Iterator<Item = T>,
        F: Fn(&T) -> String,
    {
        let options = Options::from_iter_transformed(options, transformation);
        Self::new_internal(label.into(), options)
    }

    /// Set help message to be displayed after the filter string
    pub fn help_message<S: Into<String>>(mut self, message: S) -> Self {
        self.help_message = Some(message.into());
        self
    }

    /// Makes prompt not to display the help message
    pub fn dont_display_help_message(mut self) -> Self {
        self.help_message = None;
        self
    }

    /// Sets the maximum number of options that can be displayed on the screen
    pub fn max_displayed_options(mut self, max_options: u16) -> Self {
        self.max_displayed_options = max_options;
        self
    }
}

impl<T> MultiOptionPrompt<T> for Multiselect<T> {
    fn max_options_count(&self) -> u16 {
        self.max_displayed_options
    }

    fn options(&self) -> &Options<T> {
        &self.options
    }

    fn currently_selected_index(&self) -> usize {
        self.currently_selected_index
    }

    fn draw_option(
        &self,
        option_index: usize,
        option_label: &str,
        is_selected: bool,
        commands: &mut impl CommandBuffer,
    ) {
        let is_option_selected = self.selected_options.contains(&option_index);
        self.style
            .print_option(option_label, is_option_selected, is_selected, commands);
    }

    fn draw_header(&self, commands: &mut impl CommandBuffer, is_submitted: bool) {
        if is_submitted {
            commands.set_formatting(&self.style.submitted_formatting);
            for (i, selected_index) in self.selected_options.iter().enumerate() {
                let selected_option = &self.options.transformed_options()[*selected_index];
                commands.print(selected_option);

                if i < self.selected_options.len() - 1 {
                    commands.print(", ");
                }
            }
            commands.reset_formatting();
        } else {
            commands.print(&self.filter);
            commands.print(" ");
            if let Some(help_message) = self.help_message.as_ref() {
                commands.set_formatting(&self.style.help_message_formatting);
                commands.print("[");
                commands.print(help_message);
                commands.print("]");
                commands.reset_formatting();
            }
        }
    }
}

impl<T> Prompt<Vec<T>> for Multiselect<T> {
    fn draw(&self, commands: &mut impl CommandBuffer) {
        self.draw_multioption(
            &self.label,
            self.is_submitted,
            &self.style.label_style,
            commands,
        );
    }

    fn on_key_pressed(&mut self, key: Key) -> EventOutcome<Vec<T>> {
        match key {
            Key::Up if self.currently_selected_index > 0 => {
                self.currently_selected_index -= 1;
                EventOutcome::Continue
            }
            Key::Down
                if self.currently_selected_index < self.options.filtered_options().len() - 1 =>
            {
                self.currently_selected_index += 1;
                EventOutcome::Continue
            }
            Key::Char(c) => {
                if c == ' ' {
                    let selected_option_index =
                        self.options.filtered_options()[self.currently_selected_index];
                    let existing_value_index = self
                        .selected_options
                        .iter()
                        .enumerate()
                        .find(|&x| *x.1 == selected_option_index)
                        .map(|x| x.0);

                    if let Some(i) = existing_value_index {
                        self.selected_options.remove(i);
                    } else {
                        self.selected_options.push(selected_option_index);
                    }

                    if self.filter.len() > 0 {
                        self.filter.clear();
                        self.options.filter(&self.filter);
                        self.currently_selected_index = 0;
                    }
                    EventOutcome::Continue
                } else {
                    self.filter.push(c);
                    self.options.filter(&self.filter);
                    self.currently_selected_index = 0;
                    EventOutcome::Continue
                }
            }
            Key::Backspace if self.filter.len() > 0 => {
                self.filter.pop();
                self.options.filter(&self.filter);
                self.currently_selected_index = 0;
                EventOutcome::Continue
            }
            Key::Enter if self.selected_options.len() > 0 => {
                self.is_submitted = true;
                self.selected_options.sort();

                let mut result = vec![];
                for selected_option_index in self.selected_options.iter().rev() {
                    let selected_option = self
                        .options
                        .all_options_mut()
                        .remove(*selected_option_index);
                    result.push(selected_option);
                }

                EventOutcome::Done(result)
            }
            Key::Esc => EventOutcome::Abort(AbortReason::Interrupt),
            _ => EventOutcome::Continue,
        }
    }
}

impl<T> Multiselect<T> {
    fn new_internal(label: String, options: Options<T>) -> Self {
        Multiselect {
            label,
            options,
            selected_options: vec![],
            help_message: Some(DEFAULT_HELP_MESSAGE.into()),
            max_displayed_options: DEFAUTL_MAX_OPTIONS,
            currently_selected_index: 0,
            is_submitted: false,
            filter: String::new(),
            style: MultiselectionStyle::default(),
        }
    }
}
