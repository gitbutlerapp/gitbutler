//! Tests for painting via our theme. These tests are confined as integration tests as they depend
//! on global state in the [`colored`] crate to produce colored/non-colored output.
//!
//! The tests need to run serially within this integration test as they have different requirements
//! on the control state in [`colored`].

/// This test demonstrates that each invocation of `Style.paint()` produces an "independently
/// styled" string, in the sense that the styling is prepended and a reset is appended.
#[test]
#[serial_test::serial]
fn paint_produces_self_contained_string_styling() {
    colored::control::set_override(true);

    let style = ratatui::style::Style::new()
        .fg(ratatui::style::Color::Green)
        .add_modifier(ratatui::style::Modifier::BOLD);
    let result = but::theme::Paint::paint(&style, "hello").to_string();

    let bold_green = "\x1b[1;32m";
    let reset = "\x1b[0m";
    let expected = format!("{bold_green}hello{reset}");

    assert_eq!(result, expected);

    colored::control::unset_override();
}

/// Demonstrates that `Style.paint()` respects `colored::control` for disabling colored output.
#[test]
#[serial_test::serial]
fn paint_respects_colored_control_disable() {
    colored::control::set_override(false);

    let style = ratatui::style::Style::new()
        .fg(ratatui::style::Color::Green)
        .add_modifier(ratatui::style::Modifier::BOLD);
    let result = but::theme::Paint::paint(&style, "hello").to_string();

    assert_eq!(result, "hello");

    colored::control::unset_override();
}
