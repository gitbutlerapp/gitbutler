---
name: tui-tests
description: Use when adding or modifying tests for the GitButler TUI (`but tui`) under `crates/but/src/command/legacy/status/tui/tests`.
---

## Where tests live

- Main TUI tests: `crates/but/src/command/legacy/status/tui/tests/`
- Test harness/helpers: `crates/but/src/command/legacy/status/tui/tests/utils.rs`
- Snapshots: `crates/but/src/command/legacy/status/tui/tests/snapshots/`

## Basic pattern

```rust
#[test]
fn describes_behavior_under_test() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack").unwrap();
    env.setup_metadata(&["A"]).unwrap();

    let mut tui = test_tui(env);

    tui.input_then_render(None)
        .assert_rendered_term_svg_eq(file!["snapshots/describes_behavior_under_test_001.svg"]);
}
```

## Driving the TUI

Useful input examples:

```rust
tui.input_then_render(None);                                       // render without inputs
tui.input_then_render('j');                                        // single char input
tui.input_then_render(KeyCode::Down);                              // special key
tui.input_then_render((KeyModifiers::SHIFT, KeyCode::Char('J')));  // keys with modifiers
tui.input_then_render([KeyCode::Down, KeyCode::Down]);             // multiple keys from array
tui.input_then_render("commit message text");                      // multiple keys from string
```

## Assertions

Generally prefer

- `assert_current_line_eq(str![...])` for cursor/selection behavior.
- `assert_rendered_term_svg_eq(file!["snapshots/test_function_name_001.svg"])`
  for everything else.

Generally you should include one `assert_rendered_term_svg_eq` per logical
group of inputs, to catch bad states early.

Be careful using `assert_rendered_contains` and `assert_rendered_not_contains`
since they might lead to false positives. They're intended to use while
iterating on a test where snapshots would cause too much churn.

Read `crates/but/src/command/legacy/status/tui/tests/utils.rs` for more
specialized assertions.

## Running tests

- `cargo nextest run -p but <test-name>` to run one test.
- `SNAPSHOTS=overwrite cargo nextest run -p but <test-name>` to run and update
  snapshots.
- `cargo nextest run -p but tui` to run all tui tests. Do this after changing
  things.

If a test fails the output will include the rendered state of the test backend.
This can be used when iterating on a test as a way of inspecting the state.
