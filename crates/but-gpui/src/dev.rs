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
//! click 120 300           # also dclick and tclick; append `cmd`, `alt` or `shift` to
//!                         # hold them
//! rclick 120 300
//! drag 120 300 120 500    # append `cmd`, `alt` or `shift` to hold them on letting go, and
//!                         # `hold` to keep the button down
//! drop 120 500            # let go of a held drag there
//! scroll 900 400 300      # scroll by 300 logical pixels at a window position
//! scroll 900 400 0 200    # and by 200 to the right
//! add /path/to/repo       # add a project, as Add a repository does with the folder picked
//! sh but commit -m x      # run a shell command, as an agent or another app would
//! size 900 500           # resize the window, in logical pixels
//! shot /tmp/view.png      # render the current frame to a PNG
//! perf next key down      # time an input until loads settle, plus one frame, and the
//!                         # longest the UI thread was busy meanwhile (`stall`)
//! perf startup            # time from launch until the first loads settle
//! quit
//! ```

use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use anyhow::{Context as _, Result, bail};
use gpui::{
    AnyWindowHandle, App, AppContext as _, Keystroke, Modifiers, ModifiersChangedEvent,
    MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, Pixels, PlatformInput, Point,
    ScrollDelta, ScrollWheelEvent, TouchPhase, Window, point, px,
};

use crate::app::AppView;

static LAUNCHED: OnceLock<Instant> = OnceLock::new();

/// Since when the UI thread's longest busy stretch is being kept, and how long it was. Nothing
/// can be drawn while that thread is busy, so at 120Hz no stretch should pass 8.3ms.
static STALL: Mutex<(Option<Instant>, Duration)> = Mutex::new((None, Duration::ZERO));

/// Wake on the UI thread every millisecond: a late wake is how long it was busy.
fn watch_stalls(cx: &mut App) {
    cx.spawn(async move |cx| {
        loop {
            let asleep = Instant::now();
            cx.background_executor()
                .timer(Duration::from_millis(1))
                .await;
            let mut stall = STALL.lock().expect("not poisoned");
            if let Some(since) = stall.0 {
                let late = asleep
                    .max(since)
                    .elapsed()
                    .saturating_sub(Duration::from_millis(1));
                stall.1 = stall.1.max(late);
            }
        }
    })
    .detach();
}

/// Called first thing in `main`, so `perf startup` can time from launch.
pub fn mark_launch() {
    LAUNCHED.get_or_init(Instant::now);
}

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
    // Off screen, macOS naps the app after half a minute: its timers coalesce and its
    // threads lose priority, so every step slows down. Timings should be a visible window's.
    std::mem::forget(
        objc2_foundation::NSProcessInfo::processInfo().beginActivityWithOptions_reason(
            objc2_foundation::NSActivityOptions::UserInitiated
                | objc2_foundation::NSActivityOptions::LatencyCritical,
            &objc2_foundation::NSString::from_str("Scripted run"),
        ),
    );
    watch_stalls(cx);
    // In the window, where a script sees and clicks them, not in a native alert.
    cx.set_prompt_builder(gpui::fallback_prompt_renderer);
    cx.spawn(async move |cx| {
        for (line_no, line) in script.lines().enumerate() {
            let line = line.split('#').next().unwrap_or_default().trim();
            if line.is_empty() {
                continue;
            }
            let (command, rest) = line.split_once(' ').unwrap_or((line, ""));
            // A failed step is reported and the script goes on, so it still reaches `quit`.
            let result: Result<()> = async {
                match command {
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
                    "perf" => {
                        let (label, input) = rest.split_once(' ').unwrap_or((rest, ""));
                        // Draw first so the timing doesn't include catching up on earlier steps.
                        cx.update_window(window, |_, window, cx| frame(window, cx))?;
                        *STALL.lock().expect("not poisoned") =
                            (Some(Instant::now()), Duration::ZERO);
                        let started = match label {
                            "startup" => *LAUNCHED.get().context("mark_launch wasn't called")?,
                            _ => Instant::now(),
                        };
                        let mut draw = Duration::ZERO;
                        if !input.is_empty() {
                            let (command, rest) = input.split_once(' ').unwrap_or((input, ""));
                            cx.update_window(window, |_, window, cx| {
                                dispatch(command, rest, window, cx)
                            })??;
                            // What the input shows at once, before anything it loads lands.
                            draw = frames(window, cx).await?;
                        }
                        while !cx.update_window(window, |_, window, cx| settled(window, cx))?
                            && started.elapsed() < Duration::from_secs(60)
                        {
                            cx.background_executor()
                                .timer(Duration::from_millis(1))
                                .await;
                        }
                        let data = started.elapsed();
                        let draw = draw.max(frames(window, cx).await?);
                        let stall = STALL.lock().expect("not poisoned").1.max(draw);
                        println!(
                            "PERF {label} total={:.1}ms data={:.1}ms draw={:.1}ms stall={:.1}ms",
                            (data + draw).as_secs_f64() * 1e3,
                            data.as_secs_f64() * 1e3,
                            draw.as_secs_f64() * 1e3,
                            stall.as_secs_f64() * 1e3,
                        );
                        Ok(())
                    }
                    _ => cx.update_window(window, |_, window, cx| {
                        // macOS stops drawing windows that aren't on screen; draw now so
                        // hit-testing and screenshots see the current state.
                        frame(window, cx);
                        dispatch(command, rest, window, cx)
                    })?,
                }
            }
            .await;
            if let Err(err) = result {
                eprintln!("script line {}: {line}: {err:#}", line_no + 1);
            }
            // Let the frame settle before the next step, and draw what it asked to draw again
            // (a measured height), as macOS would for a window on screen.
            cx.background_executor()
                .timer(Duration::from_millis(120))
                .await;
            cx.update_window(window, |_, window, cx| frame(window, cx))
                .ok();
        }
    })
    .detach();
}

/// Nothing loading or mutating anywhere, so the screen shows the latest state.
fn settled(window: &mut Window, cx: &mut App) -> bool {
    window.root::<AppView>().flatten().is_some_and(|app| {
        let app = app.read(cx);
        app.store.read(cx).is_settled() && app.detail.read(cx).is_settled()
    })
}

/// The frames a change asks for, as each can ask for the next, a millisecond apart: the
/// slowest of them.
async fn frames(window: AnyWindowHandle, cx: &mut gpui::AsyncApp) -> Result<Duration> {
    let mut slowest = Duration::ZERO;
    for _ in 0..4 {
        let (took, more) = cx.update_window(window, |_, window, cx| {
            let start = Instant::now();
            let more = frame(window, cx);
            (start.elapsed(), more)
        })?;
        slowest = slowest.max(took);
        if !more {
            break;
        }
        cx.background_executor()
            .timer(Duration::from_millis(1))
            .await;
    }
    Ok(slowest)
}

/// One frame as the display link would draw it: what waits for the next frame runs, then the
/// window draws. Whether anything waited, which may have asked for a frame after this one.
fn frame(window: &mut Window, cx: &mut App) -> bool {
    let waited = window.simulate_next_frame(cx) > 0;
    window.draw(cx).clear(cx);
    waited
}

fn dispatch(command: &str, rest: &str, window: &mut Window, cx: &mut App) -> Result<()> {
    let numbers = || -> Result<Vec<f32>> {
        rest.split_whitespace()
            .filter(|part| !matches!(*part, "alt" | "cmd" | "shift" | "hold"))
            .map(|part| part.parse::<f32>().context("expected a number"))
            .collect()
    };
    let held = |name| rest.split_whitespace().any(|part| part == name);
    let modifiers = Modifiers {
        platform: held("cmd"),
        alt: held("alt"),
        shift: held("shift"),
        ..Default::default()
    };
    match command {
        "key" => {
            // Hold the modifiers around the key, as a keyboard does, so code that reads
            // `window.modifiers()` sees them.
            let keystroke = Keystroke::parse(rest)?;
            let hold = |modifiers| {
                PlatformInput::ModifiersChanged(ModifiersChangedEvent {
                    modifiers,
                    capslock: Default::default(),
                })
            };
            window.dispatch_event(hold(keystroke.modifiers), cx);
            window.dispatch_keystroke(keystroke, cx);
            window.dispatch_event(hold(Modifiers::default()), cx);
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
            let at = point(px(x), px(y));
            window.dispatch_event(mouse_move(at, None, Modifiers::default()), cx);
        }
        "scroll" => {
            let (x, y, dy, dx) = match numbers()?[..] {
                [x, y, dy] => (x, y, dy, 0.),
                [x, y, dy, dx] => (x, y, dy, dx),
                _ => bail!("scroll takes x y dy, and dx to scroll sideways"),
            };
            window.dispatch_event(
                PlatformInput::ScrollWheel(ScrollWheelEvent {
                    position: point(px(x), px(y)),
                    delta: ScrollDelta::Pixels(point(px(-dx), px(-dy))),
                    modifiers: Modifiers::default(),
                    touch_phase: TouchPhase::Moved,
                }),
                cx,
            );
        }
        "click" | "rclick" | "dclick" | "tclick" => {
            let [x, y] = numbers()?[..] else {
                bail!("click takes x y");
            };
            let button = if command == "rclick" {
                MouseButton::Right
            } else {
                MouseButton::Left
            };
            // A double or triple click is the clicks before it, then one with that count.
            let clicks = match command {
                "dclick" => 2,
                "tclick" => 3,
                _ => 1,
            };
            let at = point(px(x), px(y));
            for count in 1..=clicks {
                window.dispatch_event(mouse_move(at, None, modifiers), cx);
                window.dispatch_event(mouse_down(button, at, modifiers, count), cx);
                window.dispatch_event(mouse_up(button, at, modifiers, count), cx);
            }
        }
        "drag" => {
            let [x1, y1, x2, y2] = numbers()?[..] else {
                bail!("drag takes x1 y1 x2 y2");
            };
            let (from, to) = (point(px(x1), px(y1)), point(px(x2), px(y2)));
            let left = MouseButton::Left;
            window.dispatch_event(mouse_down(left, from, Modifiers::default(), 1), cx);
            for step in 1..=8 {
                let t = step as f32 / 8.;
                let at = point(from.x + (to.x - from.x) * t, from.y + (to.y - from.y) * t);
                let modifiers = if step == 8 {
                    modifiers
                } else {
                    Modifiers::default()
                };
                window.dispatch_event(mouse_move(at, Some(left), modifiers), cx);
            }
            if !held("hold") {
                window.dispatch_event(mouse_up(left, to, modifiers, 1), cx);
            }
        }
        "release" => {
            // Let go of a drag held by `drag … hold` where the pointer is, with no move first,
            // as a hand does.
            let [x, y] = numbers()?[..] else {
                bail!("release takes x y");
            };
            let at = point(px(x), px(y));
            window.dispatch_event(mouse_up(MouseButton::Left, at, Modifiers::default(), 1), cx);
        }
        "drop" => {
            // Finish a drag left held by `drag … hold`, after frames have run.
            let [x, y] = numbers()?[..] else {
                bail!("drop takes x y");
            };
            let (at, left) = (point(px(x), px(y)), MouseButton::Left);
            window.dispatch_event(mouse_move(at, Some(left), Modifiers::default()), cx);
            window.dispatch_event(mouse_up(left, at, Modifiers::default(), 1), cx);
        }
        // What the system's folder picker would hand Add a repository.
        "add" => {
            let app = window.root::<AppView>().flatten().context("no app view")?;
            app.update(cx, |app, cx| {
                app.store
                    .update(cx, |store, cx| store.add_project(PathBuf::from(rest), cx))
            });
        }
        // What an agent, another app or the user does outside the app meanwhile.
        "sh" => {
            let status = std::process::Command::new("sh")
                .args(["-c", rest])
                .status()?;
            if !status.success() {
                bail!("`{rest}` failed: {status}");
            }
        }
        "size" => {
            let [width, height] = numbers()?[..] else {
                bail!("size takes width height");
            };
            window.resize(gpui::size(px(width), px(height)));
        }
        "shot" => {
            let image = window.render_to_image()?;
            image.save(rest).with_context(|| format!("saving {rest}"))?;
        }
        other => bail!("unknown command {other}"),
    }
    Ok(())
}

fn mouse_move(
    position: Point<Pixels>,
    pressed_button: Option<MouseButton>,
    modifiers: Modifiers,
) -> PlatformInput {
    PlatformInput::MouseMove(MouseMoveEvent {
        position,
        pressed_button,
        modifiers,
    })
}

fn mouse_down(
    button: MouseButton,
    position: Point<Pixels>,
    modifiers: Modifiers,
    click_count: usize,
) -> PlatformInput {
    PlatformInput::MouseDown(MouseDownEvent {
        button,
        position,
        modifiers,
        click_count,
        first_mouse: false,
    })
}

fn mouse_up(
    button: MouseButton,
    position: Point<Pixels>,
    modifiers: Modifiers,
    click_count: usize,
) -> PlatformInput {
    PlatformInput::MouseUp(MouseUpEvent {
        button,
        position,
        modifiers,
        click_count,
    })
}
