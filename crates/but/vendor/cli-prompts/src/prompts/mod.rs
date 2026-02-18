//! The module that contains the high-level abstractions for the prompts. 
//!
//! To implement a new prompt, you need to implement the `Prompt` trait, which 
//! requires a method to draw the prompt and to react to input. Having implemented
//! this trait, you will be able to call `display()` on your prompt object which 
//! handle the rest

mod confirmation;
mod input;
mod options;

pub use confirmation::Confirmation;
pub use input::Input;
pub use options::multiselect::Multiselect;
pub use options::selection::Selection;
pub use options::{Options, multioption_prompt::MultiOptionPrompt};

use std::io::stdout;

use crate::{
    engine::{Clear, CommandBuffer, CrosstermEngine, Engine},
    input::Key,
};

/// Describes the reason for prompt abortion
#[derive(Debug)]
pub enum AbortReason {

    /// The prompt was interrupted by the user
    Interrupt,

    /// The error occured with I/O
    Error(std::io::Error),
}

/// This describes the final result of the prompt
#[derive(Debug)]
pub enum EventOutcome<T> {

    /// The prompt has successfully completed.
    /// The inner field will contain the result.
    Done(T),

    /// This signals that the prompt hasn't completed and should continue
    Continue,

    /// Prompt has been aborted
    Abort(AbortReason),
}

/// The trait for defining interactive prompts.
///
/// ```rust
///use cli_prompts::{
///   prompts::{Prompt, EventOutcome, AbortReason},
///   engine::CommandBuffer,
///   style::{Formatting, Color},
///   input::Key,
///};
///
///struct MyPrompt {
///   name: String
///}
///
/// impl Prompt<String> for MyPrompt {
///    fn draw(&self, commands: &mut impl CommandBuffer) {
///       commands.print("Input your name: ");
///       commands.set_formatting(&Formatting::default().foreground_color(Color::Green));
///       commands.print(&self.name);
///       commands.reset_formatting();
///    }
///
///    fn on_key_pressed(&mut self, key: Key) -> EventOutcome<String> {
///           match key {
///               Key::Char(c) => {
///               self.name.push(c);
///               EventOutcome::Continue
///           },
///           Key::Backspace => {
///               self.name.pop();
///               EventOutcome::Continue
///           },
///           Key::Enter => EventOutcome::Done(self.name.clone()),
///           Key::Esc => EventOutcome::Abort(AbortReason::Interrupt),
///           _ => EventOutcome::Continue,
///       }
///    }
/// }
/// ```
pub trait Prompt<TOut> {

    /// Defines how to draw the prompt with a set of commands.
    /// The goal of this method is to populate the `commands` buffer
    /// with a set of commands that will draw your prompt to the screen.
    fn draw(&self, commands: &mut impl CommandBuffer);

    /// This should handle the keyboard key presses. Should return the 
    /// outcome of the keypress:
    /// - EventOutcome::Continue - the input was handled and the prompt should continue displaying
    /// - EventOutcome::Done(TOut) - the prompt has successfully completed. Pass the result as the
    /// enum's field
    /// - EventOutcome::Abort(AbortReason) - the prompt has finished abruptly. Specify a reason in
    /// the enum's field
    fn on_key_pressed(&mut self, key: Key) -> EventOutcome<TOut>;
}

/// A trait that is implemented for every type that implements `Prompt`. Provides a convenient way
/// to display a prompt on the screen, handle the input, raw mode, etc.
pub trait DisplayPrompt<T> {

    /// Draws the prompt on the screen and handles the input.
    /// - Returns `Ok(T)` if the prompt is completed successfully.
    /// - Returns `Err(AbortReason)` if it failed. Check the `AbortReason` to find out why
    fn display(self) -> Result<T, AbortReason>;
}

impl<T, P> DisplayPrompt<T> for P
where
    P: Prompt<T> + Sized,
{
    fn display(mut self) -> Result<T, AbortReason> {
        let buffer = stdout();
        let mut engine = CrosstermEngine::new(buffer);
        let mut commands = engine.get_command_buffer();

        loop {
            self.draw(&mut commands);
            engine.render(&commands)?;

            let key_pressed = engine.read_key()?;
            match self.on_key_pressed(key_pressed) {
                EventOutcome::Done(result) => {
                    commands.clear();
                    self.draw(&mut commands);
                    engine.render(&commands)?;
                    engine.finish_rendering()?;

                    return Ok(result);
                }
                EventOutcome::Continue => {
                    commands.clear();
                    continue;
                }
                EventOutcome::Abort(reason) => return Err(reason),
            }
        }
    }
}

impl From<std::io::Error> for AbortReason {
    fn from(error: std::io::Error) -> Self {
        AbortReason::Error(error)
    }
}
