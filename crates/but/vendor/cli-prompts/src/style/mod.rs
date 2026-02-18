//! Module for changing the appearence of the prompts.
//! Each prompt has its own set of styling attributes, which include text color, different
//! formatting as well as some prompt-specific styling

mod color;
mod formatting;
mod label_style;
mod prompts;

pub use color::Color;
pub use formatting::{Formatting, FormattingOption};
pub use label_style::LabelStyle;
pub use prompts::{
    confirmation::ConfirmationStyle,
    input::InputStyle,
    multiselection::MultiselectionStyle,
    selection::{self, SelectionStyle},
};
