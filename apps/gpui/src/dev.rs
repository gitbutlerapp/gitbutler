//! Scripted input and screenshots, for checking the UI without a person at the keyboard.
//! Built only with `--features dev-script`; run with `GITBUTLER_GPUI_SCRIPT=<file>`.
//!
//! One command per line, `#` starts a comment:
//!
//! ```text
//! wait 500                # milliseconds
//! key cmd-shift-n         # a keystroke, as in key bindings
//! type hello world        # text, one keystroke per character
//! move 120 300            # window coordinates in logical pixels
//! click 120 300
//! rclick 120 300
//! drag 120 300 120 500    # append `alt` to hold option, `hold` to keep the button down
//! shot /tmp/view.png      # render the current frame to a PNG
//! quit
//! ```

use std::path::PathBuf;
use std::time::Duration;

use anyhow::{Context as _, Result, bail};
use gpui::{
    AnyWindowHandle, App, AppContext as _, Keystroke, Modifiers, MouseButton, MouseDownEvent,
    MouseMoveEvent, MouseUpEvent, PlatformInput, Point, Window, point, px,
};

pub fn run_from_env(window: AnyWindowHandle, cx: &mut App) {
    let Some(path) = std::env::var_os("GITBUTLER_GPUI_SCRIPT").map(PathBuf::from) else {
        return;
    };
    let script = match std::fs::read_to_string(&path) {
        Ok(script) => script,
        Err(err) => {
            eprintln!("can't read {}: {err}", path.display());
            return;
        }
    };
    cx.spawn(async move |cx| {
        for (line_no, line) in script.lines().enumerate() {
            let line = line.split('#').next().unwrap_or_default().trim();
            if line.is_empty() {
                continue;
            }
            let (command, rest) = line.split_once(' ').unwrap_or((line, ""));
            let result: Result<()> = match command {
                "wait" => {
                    let ms: u64 = rest.parse().context("wait takes milliseconds")?;
                    cx.background_executor()
                        .timer(Duration::from_millis(ms))
                        .await;
                    Ok(())
                }
                "quit" => {
                    cx.update(|cx| cx.quit());
                    Ok(())
                }
                _ => cx
                    .update_window(window, |_, window, cx| run(command, rest, window, cx))
                    .and_then(|result| result),
            };
            if let Err(err) = result {
                eprintln!("script line {}: {line}: {err:#}", line_no + 1);
            }
            // Let the frame settle before the next step.
            cx.background_executor()
                .timer(Duration::from_millis(120))
                .await;
        }
        anyhow::Ok(())
    })
    .detach();
}

fn run(command: &str, rest: &str, window: &mut Window, cx: &mut App) -> Result<()> {
    // macOS stops drawing windows that aren't on screen; draw now so hit-testing and
    // screenshots see the current state.
    window.draw(cx).clear(cx);
    let numbers = || -> Result<Vec<f32>> {
        rest.split_whitespace()
            .filter(|part| !matches!(*part, "alt" | "hold"))
            .map(|part| part.parse::<f32>().context("expected a number"))
            .collect()
    };
    match command {
        "key" => {
            window.dispatch_keystroke(Keystroke::parse(rest)?, cx);
        }
        "type" => {
            for ch in rest.chars() {
                let key = if ch == ' ' {
                    "space".to_owned()
                } else {
                    ch.to_string()
                };
                let mut keystroke = Keystroke::parse(&key)?;
                keystroke.key_char = Some(ch.to_string());
                window.dispatch_keystroke(keystroke, cx);
            }
        }
        "move" => {
            let [x, y] = numbers()?[..] else {
                bail!("move takes x y");
            };
            window.dispatch_event(
                PlatformInput::MouseMove(MouseMoveEvent {
                    position: point(px(x), px(y)),
                    pressed_button: None,
                    modifiers: Modifiers::default(),
                }),
                cx,
            );
        }
        "click" | "rclick" => {
            let [x, y] = numbers()?[..] else {
                bail!("click takes x y");
            };
            let button = if command == "click" {
                MouseButton::Left
            } else {
                MouseButton::Right
            };
            press(
                window,
                cx,
                point(px(x), px(y)),
                button,
                Modifiers::default(),
            );
        }
        "drag" => {
            let [x1, y1, x2, y2] = numbers()?[..] else {
                bail!("drag takes x1 y1 x2 y2");
            };
            let release_modifiers = Modifiers {
                alt: rest.split_whitespace().any(|part| part == "alt"),
                ..Default::default()
            };
            let (from, to) = (point(px(x1), px(y1)), point(px(x2), px(y2)));
            window.dispatch_event(
                PlatformInput::MouseDown(MouseDownEvent {
                    button: MouseButton::Left,
                    position: from,
                    modifiers: Modifiers::default(),
                    click_count: 1,
                    first_mouse: false,
                }),
                cx,
            );
            for step in 1..=8 {
                let t = step as f32 / 8.;
                let at = point(from.x + (to.x - from.x) * t, from.y + (to.y - from.y) * t);
                window.dispatch_event(
                    PlatformInput::MouseMove(MouseMoveEvent {
                        position: at,
                        pressed_button: Some(MouseButton::Left),
                        modifiers: if step == 8 {
                            release_modifiers
                        } else {
                            Modifiers::default()
                        },
                    }),
                    cx,
                );
            }
            if !rest.split_whitespace().any(|part| part == "hold") {
                window.dispatch_event(
                    PlatformInput::MouseUp(MouseUpEvent {
                        button: MouseButton::Left,
                        position: to,
                        modifiers: release_modifiers,
                        click_count: 1,
                    }),
                    cx,
                );
            }
        }
        "shot" => {
            let image = window.render_to_image()?;
            image.save(rest).with_context(|| format!("saving {rest}"))?;
        }
        other => bail!("unknown command {other}"),
    }
    Ok(())
}

fn press(
    window: &mut Window,
    cx: &mut App,
    position: Point<gpui::Pixels>,
    button: MouseButton,
    modifiers: Modifiers,
) {
    window.dispatch_event(
        PlatformInput::MouseMove(MouseMoveEvent {
            position,
            pressed_button: None,
            modifiers,
        }),
        cx,
    );
    window.dispatch_event(
        PlatformInput::MouseDown(MouseDownEvent {
            button,
            position,
            modifiers,
            click_count: 1,
            first_mouse: false,
        }),
        cx,
    );
    window.dispatch_event(
        PlatformInput::MouseUp(MouseUpEvent {
            button,
            position,
            modifiers,
            click_count: 1,
        }),
        cx,
    );
}
