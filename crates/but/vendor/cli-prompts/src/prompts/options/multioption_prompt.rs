use crate::{engine::CommandBuffer, prompts::options::Options, style::LabelStyle};

/// Helper trait that simplifies the implementation of the prompts that have multiple options to
/// choose from. It handles filtering, pagination, drawing prompt header and options.
pub trait MultiOptionPrompt<T> {

    /// Maximum number of options that can be displayed on the screen
    fn max_options_count(&self) -> u16;

    /// Returns the reference to the `Options` struct
    fn options(&self) -> &Options<T>;

    /// Get the index of the option on the screen that is currently selected
    fn currently_selected_index(&self) -> usize;

    /// Draws the option with the given index and label
    fn draw_option(
        &self,
        option_index: usize,
        option_label: &str,
        is_selected: bool,
        cmd_buffer: &mut impl CommandBuffer,
    );

    /// Draws the prompt header
    fn draw_header(&self, cmd_buffer: &mut impl CommandBuffer, is_submitted: bool);

    /// Draws the entire prompt with all the options. Call this from within the `Prompt::draw()`
    /// method
    fn draw_multioption(
        &self,
        label: &str,
        is_submitted: bool,
        label_style: &LabelStyle,
        cmd_buffer: &mut impl CommandBuffer,
    ) {
        label_style.print(label, cmd_buffer);
        self.draw_header(cmd_buffer, is_submitted);

        if is_submitted {
            return;
        }

        cmd_buffer.new_line();
        let max_options_count: usize = self.max_options_count().into();
        let mut start_from = self
            .currently_selected_index()
            .checked_sub(max_options_count / 2)
            .unwrap_or(0);
        start_from = start_from.min(
            self.options()
                .filtered_options()
                .len()
                .checked_sub(max_options_count)
                .unwrap_or(0),
        );

        let displayed_option_indices = self
            .options()
            .filtered_options()
            .iter()
            .enumerate()
            .skip(start_from)
            .take(self.max_options_count().into());

        for (selection_index, option_index) in displayed_option_indices {
            let is_selected = selection_index == self.currently_selected_index();
            let option_label = &self.options().transformed_options()[*option_index];

            self.draw_option(*option_index, option_label, is_selected, cmd_buffer);
            cmd_buffer.new_line();
        }
    }
}
