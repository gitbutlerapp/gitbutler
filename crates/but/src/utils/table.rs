use crate::utils::table::types::Table;
use std::io::Write;
use terminal_size::Width;
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

/// Avoid paths like `table::Table` when importing.
pub(super) mod types {
    use crate::utils::table::Cell;

    /// A simple table formatter that can render tables with fixed-width columns
    pub struct Table {
        pub(super) headers: Vec<Cell>,
        pub(super) rows: Vec<Vec<Cell>>,
        pub(super) terminal_width: usize,
    }
}

/// Represents a single cell in a table
#[derive(Debug, Clone)]
pub struct Cell {
    pub content: String,
    pub width: Option<usize>,
    pub align: Alignment,
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
        }
    }

    pub fn with_width(mut self, width: usize) -> Self {
        self.width = Some(width);
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
        }
    }

    pub fn add_row(&mut self, row: Vec<Cell>) {
        self.rows.push(row);
    }

    pub fn render<W: Write>(&self, writer: &mut W) -> std::io::Result<()> {
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
            self.render_row(writer, row, &column_widths)?;
            writeln!(writer)?;
        }

        Ok(())
    }

    fn render_row<W: Write>(
        &self,
        writer: &mut W,
        cells: &[Cell],
        column_widths: &[usize],
    ) -> std::io::Result<()> {
        for (i, cell) in cells.iter().enumerate() {
            if i > 0 {
                write!(writer, " ")?;
            }

            let width = column_widths.get(i).copied().unwrap_or(0);
            let formatted = format_cell(&cell.content, width, cell.align);
            write!(writer, "{}", formatted)?;
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

        // If we exceed terminal width, shrink columns proportionally
        if total_fixed_width > self.terminal_width {
            // Find columns without explicit widths (flexible columns)
            let flexible_indices: Vec<usize> = self
                .headers
                .iter()
                .enumerate()
                .filter_map(|(i, h)| if h.width.is_none() { Some(i) } else { None })
                .collect();

            if !flexible_indices.is_empty() {
                let fixed_width: usize = self.headers.iter().filter_map(|h| h.width).sum();

                let available_for_flexible = self
                    .terminal_width
                    .saturating_sub(fixed_width + separator_width);
                let per_flexible = available_for_flexible / flexible_indices.len();

                for &idx in &flexible_indices {
                    widths[idx] = widths[idx].min(per_flexible);
                }
            }
        }

        widths
    }
}

fn format_cell(content: &str, width: usize, align: Alignment) -> String {
    let stripped = strip_ansi_codes(content);
    let content_width = stripped.width();

    if content_width >= width {
        // Truncate with ellipsis if too long
        if width <= 3 {
            return truncate_string(content, width);
        }
        return truncate_string(content, width.saturating_sub(1)) + "…";
    }

    // Calculate padding
    let padding_needed = width.saturating_sub(content_width);

    match align {
        Alignment::Left => format!("{}{}", content, " ".repeat(padding_needed)),
    }
}

fn truncate_string(s: &str, max_width: usize) -> String {
    // This is a simplified version that handles ANSI codes
    let mut result = String::new();
    let mut current_width = 0;
    let mut in_ansi = false;
    let mut ansi_buffer = String::new();

    for ch in s.chars() {
        if ch == '\x1b' {
            in_ansi = true;
            ansi_buffer.push(ch);
            continue;
        }

        if in_ansi {
            ansi_buffer.push(ch);
            if ch == 'm' {
                result.push_str(&ansi_buffer);
                ansi_buffer.clear();
                in_ansi = false;
            }
            continue;
        }

        let char_width = ch.width().unwrap_or(0);
        if current_width + char_width > max_width {
            break;
        }

        result.push(ch);
        current_width += char_width;
    }

    result
}

fn strip_ansi_codes(s: &str) -> String {
    let mut result = String::new();
    let mut in_escape = false;

    for ch in s.chars() {
        if ch == '\x1b' {
            in_escape = true;
            continue;
        }

        if in_escape {
            if ch == 'm' {
                in_escape = false;
            }
            continue;
        }

        result.push(ch);
    }

    result
}

fn terminal_width() -> usize {
    terminal_size::terminal_size().map_or(80, |(Width(w), _)| w as usize)
}
