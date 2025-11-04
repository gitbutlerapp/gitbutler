use ratatui::{buffer::Buffer, layout::Position};

#[allow(dead_code)]
/// Convert a ratatui Buffer into a readable string for snapshot testing
pub fn buffer_to_string(buf: &Buffer) -> String {
    let mut output = String::new();
    for y in 0..buf.area.height {
        for x in 0..buf.area.width {
            let Some(cell) = buf.cell(Position::new(x, y)) else {
                output.push(' ');
                continue;
            };
            output.push(cell.symbol().chars().next().unwrap_or(' '));
        }
        output.push('\n');
    }
    output
}
