pub mod input {
    use crate::style::{Color, Formatting, LabelStyle};

    /// Style for the `Input` prompt
    pub struct InputStyle {
        /// Style of the prompt itself
        pub label_style: LabelStyle,

        /// Formatting for the default value text
        pub default_value_formatting: Formatting,

        /// Formatting for the error message
        pub error_formatting: Formatting,

        /// Formatting for the user's input
        pub input_formatting: Formatting,

        /// Formatting for the user's input when the prompt is completed
        pub submitted_formatting: Formatting,

        /// Formatting for the help message
        pub help_message_formatting: Formatting,
    }

    impl Default for InputStyle {
        fn default() -> Self {
            InputStyle {
                label_style: LabelStyle::default(),
                default_value_formatting: Formatting::default().foreground_color(Color::Grey),
                error_formatting: Formatting::default().foreground_color(Color::Red),
                input_formatting: Formatting::default(),
                submitted_formatting: Formatting::default().foreground_color(Color::Green),
                help_message_formatting: Formatting::default().foreground_color(Color::DarkGreen),
            }
        }
    }

    impl InputStyle {
        pub fn label_style(mut self, l: LabelStyle) -> Self {
            self.label_style = l;
            self
        }

        pub fn default_value_formatting(mut self, f: Formatting) -> Self {
            self.default_value_formatting = f;
            self
        }

        pub fn error_formatting(mut self, f: Formatting) -> Self {
            self.error_formatting = f;
            self
        }

        pub fn input_formatting(mut self, f: Formatting) -> Self {
            self.input_formatting = f;
            self
        }

        pub fn submitted_formatting(mut self, f: Formatting) -> Self {
            self.submitted_formatting = f;
            self
        }

        pub fn help_message_formatting(mut self, f: Formatting) -> Self {
            self.help_message_formatting = f;
            self
        }
    }
}

pub mod confirmation {
    use crate::style::{Color, Formatting, LabelStyle};

    /// Style for the `Confirmation` prompt
    pub struct ConfirmationStyle {
        /// Style for the prompt itself
        pub label_style: LabelStyle,
        
        /// Style for the user's input
        pub input_formatting: Formatting,

        /// Formatting for the user's input when the prompt is completed
        pub submitted_formatting: Formatting,
    }

    impl Default for ConfirmationStyle {
        fn default() -> Self {
            ConfirmationStyle {
                label_style: LabelStyle::default(),
                input_formatting: Formatting::default(),
                submitted_formatting: Formatting::default().foreground_color(Color::Green),
            }
        }
    }

    impl ConfirmationStyle {
        pub fn label_style(mut self, l: LabelStyle) -> Self {
            self.label_style = l;
            self
        }

        pub fn input_formatting(mut self, f: Formatting) -> Self {
            self.input_formatting = f;
            self
        }

        pub fn submitted_formatting(mut self, f: Formatting) -> Self {
            self.submitted_formatting = f;
            self
        }
    }
}

pub mod selection {
    use crate::{
        engine::CommandBuffer,
        style::{Color, Formatting, LabelStyle},
    };

    /// Marker that is displayed before the option that is currently highlighted
    pub struct Marker {
        /// Marker string
        pub marker: String,

        /// Formatting of the marker
        pub formatting: Formatting,
    }

    /// Style for the `Selection` prompt
    pub struct SelectionStyle {
        /// Style for the prompt itself
        pub label_style: LabelStyle,

        /// Formatting for the user's input when the prompt is completed
        pub submitted_formatting: Formatting,

        /// Formatting for the options
        pub option_formatting: Formatting,

        /// Formatting for the option that is currently highlighted
        pub selected_option_formatting: Formatting,

        /// Formatting for the filter string
        pub filter_formatting: Formatting,

        /// Marker for the option which is not highlighted
        pub not_selected_marker: Marker,

        /// Marker for the option which is currently highlighted
        pub selected_marker: Marker,
    }

    impl Default for SelectionStyle {
        fn default() -> Self {
            SelectionStyle {
                label_style: LabelStyle::default(),
                submitted_formatting: Formatting::default().foreground_color(Color::Green),
                option_formatting: Formatting::default(),
                selected_option_formatting: Formatting::default().bold(),
                filter_formatting: Formatting::default(),
                not_selected_marker: Marker {
                    marker: "  ".into(),
                    formatting: Formatting::default(),
                },
                selected_marker: Marker {
                    marker: "> ".into(),
                    formatting: Formatting::default().bold(),
                },
            }
        }
    }

    impl SelectionStyle {
        pub fn label_style(mut self, l: LabelStyle) -> Self {
            self.label_style = l;
            self
        }

        pub fn submitted_formatting(mut self, f: Formatting) -> Self {
            self.submitted_formatting = f;
            self
        }

        pub fn option_formatting(mut self, f: Formatting) -> Self {
            self.option_formatting = f;
            self
        }

        pub fn not_selected_marker(mut self, m: Marker) -> Self {
            self.not_selected_marker = m;
            self
        }

        pub fn selected_marker(mut self, m: Marker) -> Self {
            self.selected_marker = m;
            self
        }
    }

    impl Marker {
        pub fn print(&self, cmd_buffer: &mut impl CommandBuffer) {
            cmd_buffer.set_formatting(&self.formatting);
            cmd_buffer.print(&self.marker);
            cmd_buffer.reset_formatting();
        }
    }
}

pub mod multiselection {
    use crate::{
        engine::CommandBuffer,
        style::{Color, Formatting, LabelStyle},
    };

    /// Style for the `Multiselection` prompt
    pub struct MultiselectionStyle {
        /// Style for the prompt itself
        pub label_style: LabelStyle,

        /// Formatting for the user's input when the prompt is completed
        pub submitted_formatting: Formatting,

        /// Formatting for the filter string
        pub filter_formatting: Formatting,


        /// Formatting for the help message
        pub help_message_formatting: Formatting,

        /// Marker to use
        pub marker: Marker,

        /// Formatting for the option that is currently highlighted
        pub highlighted_option_formatting: Formatting,

        /// Formatting for the option which is not currently highlighted
        pub normal_option_formatting: Formatting,
    }

    /// Marker for the options. It consists of the opening and closing symbols and the symbol that
    /// is put in the middle when the option is selected. Example: 
    /// Not selected: [ ]
    /// Selected:     [X]
    pub struct Marker {
        /// Opening symbol
        pub opening_sign: String,

        /// Closing symbol
        pub closing_sign: String,

        /// The symbol that is put between the above two when the option is selected
        pub selection_sign: String,
    }

    impl Default for MultiselectionStyle {
        fn default() -> Self {
            MultiselectionStyle {
                label_style: LabelStyle::default(),
                submitted_formatting: Formatting::default().foreground_color(Color::Green),
                filter_formatting: Formatting::default(),
                help_message_formatting: Formatting::default().foreground_color(Color::DarkGreen),
                marker: Marker {
                    opening_sign: "[".into(),
                    selection_sign: "x".into(),
                    closing_sign: "]".into(),
                },
                highlighted_option_formatting: Formatting::default()
                    .foreground_color(Color::DarkGreen),
                normal_option_formatting: Formatting::default(),
            }
        }
    }

    impl Marker {
        /// Prints the marker to the provided command buffer
        pub fn print(&self, is_selected: bool, commands: &mut impl CommandBuffer) {
            let sign = if is_selected {
                &self.selection_sign
            } else {
                " "
            };
            commands.print(&format!(
                "{}{}{}",
                self.opening_sign, sign, self.closing_sign
            ));
        }
    }

    impl MultiselectionStyle {

        /// Prints the option with the given text along with the marker to the provided command
        /// buffer
        pub fn print_option(
            &self,
            option_text: &str,
            is_selected: bool,
            is_highlighted: bool,
            commands: &mut impl CommandBuffer,
        ) {
            let formatting = if is_highlighted {
                &self.highlighted_option_formatting
            } else {
                &self.normal_option_formatting
            };

            commands.set_formatting(formatting);
            self.marker.print(is_selected, commands);
            commands.print(" ");

            commands.print(option_text);
            commands.reset_formatting();
        }
    }
}
