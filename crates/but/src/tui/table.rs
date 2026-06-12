//! A generic table for displaying aligned rows.
use unicode_width::UnicodeWidthStr;

use super::text::{strip_ansi_codes, terminal_width, truncate_text};

/// Avoid paths like `table::Table` when importing.
pub(super) mod types {
    use crate::tui::table::Cell;

    /// A simple table formatter that can render tables with fixed-width columns
    pub struct Table {
        pub(super) headers: Vec<Cell>,
        pub(super) rows: Vec<Vec<Cell>>,
        pub(super) terminal_width: usize,
        pub(super) allow_truncation: bool,
    }
}
use types::Table;

/// Represents a single cell in a table
#[derive(Debug, Clone)]
pub struct Cell {
    pub content: String,
    pub width: Option<usize>,
    pub align: Alignment,
    /// If true, this column will never be truncated when shrinking to fit the terminal.
    /// Other flexible columns will be shrunk first.
    pub no_truncate: bool,
}

#[derive(Debug, Clone, Copy)]
pub enum Alignment {
    Left,
}

impl Cell {
    pub fn new(content: impl Into<String>) -> Self {
        Self {
            content: content.into(),
            width: None,
            align: Alignment::Left,
            no_truncate: false,
        }
    }

    pub fn with_width(mut self, width: usize) -> Self {
        self.width = Some(width);
        self
    }

    /// Mark this column as never truncatable. Other flexible columns will shrink first.
    pub fn no_truncate(mut self) -> Self {
        self.no_truncate = true;
        self
    }
}

impl Table {
    pub fn new(headers: Vec<Cell>) -> Self {
        let terminal_width = terminal_width();
        Self {
            headers,
            rows: Vec::new(),
            terminal_width,
            allow_truncation: true,
        }
    }

    pub fn with_truncation(mut self, allow_truncation: bool) -> Self {
        self.allow_truncation = allow_truncation;
        self
    }

    pub fn add_row(&mut self, row: Vec<Cell>) {
        self.rows.push(row);
    }

    pub fn render<W: std::fmt::Write + ?Sized>(&self, out: &mut W) -> std::fmt::Result {
        if self.headers.is_empty() {
            return Ok(());
        }

        // Calculate column widths
        let column_widths = self.calculate_column_widths();

        // Render header (optional, can be commented out for no header)
        // self.render_row(writer, &self.headers, &column_widths)?;
        // writeln!(writer)?;

        // Render rows
        for row in &self.rows {
            self.render_row(out, row, &column_widths)?;
            writeln!(out)?;
        }

        Ok(())
    }

    fn render_row<W: std::fmt::Write + ?Sized>(
        &self,
        out: &mut W,
        cells: &[Cell],
        column_widths: &[usize],
    ) -> std::fmt::Result {
        for (i, cell) in cells.iter().enumerate() {
            if i > 0 {
                write!(out, " ")?;
            }

            let width = column_widths.get(i).copied().unwrap_or(0);
            let formatted = format_cell(&cell.content, width, cell.align, self.allow_truncation);
            write!(out, "{formatted}")?;
        }
        Ok(())
    }

    fn calculate_column_widths(&self) -> Vec<usize> {
        let num_columns = self.headers.len();
        let mut widths = vec![0; num_columns];

        // Get explicitly set widths or measure content
        for (i, header) in self.headers.iter().enumerate() {
            if let Some(w) = header.width {
                widths[i] = w;
            } else {
                // Measure header
                widths[i] = strip_ansi_codes(&header.content).width();

                // Measure all rows
                for row in &self.rows {
                    if let Some(cell) = row.get(i) {
                        let content_width = strip_ansi_codes(&cell.content).width();
                        widths[i] = widths[i].max(content_width);
                    }
                }
            }
        }

        // Calculate total width needed
        let separator_width = num_columns.saturating_sub(1); // spaces between columns
        let total_fixed_width: usize = widths.iter().sum::<usize>() + separator_width;

        // If we exceed terminal width, shrink flexible columns to fit.
        // Columns marked `no_truncate` keep their full content width;
        // only other flexible columns are shrunk.
        if self.allow_truncation && total_fixed_width > self.terminal_width {
            let flexible_indices: Vec<usize> = self
                .headers
                .iter()
                .enumerate()
                .filter_map(|(i, h)| if h.width.is_none() { Some(i) } else { None })
                .collect();

            if !flexible_indices.is_empty() {
                let fixed_width: usize = self
                    .headers
                    .iter()
                    .enumerate()
                    .filter_map(|(i, h)| {
                        if h.width.is_some() {
                            Some(widths[i])
                        } else {
                            None
                        }
                    })
                    .sum();

                let available_for_flexible = self
                    .terminal_width
                    .saturating_sub(fixed_width + separator_width);

                // Separate no_truncate columns from shrinkable ones
                let no_truncate_width: usize = flexible_indices
                    .iter()
                    .filter(|&&i| self.headers[i].no_truncate)
                    .map(|&i| widths[i])
                    .sum();

                let shrinkable_indices: Vec<usize> = flexible_indices
                    .iter()
                    .filter(|&&i| !self.headers[i].no_truncate)
                    .copied()
                    .collect();

                if !shrinkable_indices.is_empty() {
                    let available_for_shrinkable =
                        available_for_flexible.saturating_sub(no_truncate_width);
                    let per_shrinkable = available_for_shrinkable / shrinkable_indices.len();

                    for &idx in &shrinkable_indices {
                        widths[idx] = widths[idx].min(per_shrinkable);
                    }
                }
            }
        }

        widths
    }
}

fn format_cell(content: &str, width: usize, align: Alignment, allow_truncation: bool) -> String {
    let stripped = strip_ansi_codes(content);
    let content_width = stripped.width();

    if allow_truncation && content_width >= width {
        return truncate_text(content, width).into_owned();
    }

    // Calculate padding
    let padding_needed = width.saturating_sub(content_width);

    match align {
        Alignment::Left => format!("{}{}", content, " ".repeat(padding_needed)),
    }
}

#[cfg(test)]
mod tests {
    use super::{Cell, Table};

    #[test]
    fn render_truncates_cells_by_default() {
        let mut table = Table::new(vec![Cell::new("NAME").with_width(5)]);
        table.add_row(vec![Cell::new("hello world")]);

        let mut output = String::new();
        table.render(&mut output).unwrap();

        assert_eq!(output, "hell…\n");
    }

    #[test]
    fn render_keeps_full_cells_when_truncation_is_disabled() {
        let mut table = Table::new(vec![Cell::new("NAME").with_width(5)]).with_truncation(false);
        table.add_row(vec![Cell::new("hello world")]);

        let mut output = String::new();
        table.render(&mut output).unwrap();

        assert_eq!(output, "hello world\n");
    }
}
