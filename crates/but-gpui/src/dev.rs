//! Scripted input and screenshots, for checking the UI without a person at the keyboard.
//! Built only with `--features dev-script`; run with `GITBUTLER_GPUI_SCRIPT=<file>`, or with
//! `scripts/drive.sh`, which builds it and keeps the run in a sandbox. The window never shows,
//! the app stays out of the Dock and never takes the keys from the app in front, and the run
//! quits when the script ends.
//!
//! One command per line, `#` starts a comment:
//!
//! ```text
//! wait 500                # milliseconds
//! key cmd-shift-n         # a keystroke, as in key bindings
//! type hello world        # text, one keystroke per character
//! move 120 300            # window coordinates in logical pixels; with the button held
//!                         # after `press` or `drag … hold`, a drag's move
//! click 120 300           # also dclick and tclick; append `cmd`, `alt` or `shift` to
//!                         # hold them
//! rclick 120 300
//! press 120 300           # press the left button there and keep it down, so `move`s
//!                         # drag with frames between them, as a hand does
//! drag 120 300 120 500    # append `cmd`, `alt` or `shift` to hold them on letting go, and
//!                         # `hold` to keep the button down
//! drop 120 500            # let go of a held drag there
//! release 120 500         # let go without moving there first, as a hand does
//! scroll 900 400 300      # scroll by 300 logical pixels at a window position
//! scroll 900 400 0 200    # and by 200 to the right
//! add /path/to/repo       # add a project, as Add a repository does with the folder picked
//! sh but commit -m x      # run a shell command, as an agent or another app would
//! size 900 500           # resize the window, in logical pixels
//! shot /tmp/view.png      # render the current frame to a PNG, a pixel per logical pixel, so
//!                         # a point read off it is one `click` takes
//! shot /tmp/a.png 0 0 400 300  # only that region, in logical pixels, at the display's scale
//! keys                    # print what typing would reach, as `KEYS Workspace > Navigator`
//! perf next key down      # time an input until loads settle, plus one frame, and the
//!                         # longest the UI thread was busy meanwhile (`stall`)
//! perf startup            # time from launch until the first loads settle
//! hold nav down 40 33      # hold a key: press it 40 times, 33ms apart as a held key repeats,
//!                         # each drawn before the next; prints how long the presses took to
//!                         # draw and the longest stall
//! quit
//! ```

use std::path::PathBuf;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use anyhow::{Context as _, Result, bail};
use gpui::{
    AnyWindowHandle, App, AppContext as _, KeyUpEvent, Keystroke, Modifiers, ModifiersChangedEvent,
    MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, Pixels, PlatformInput, Point,
    ScrollDelta, ScrollWheelEvent, TouchPhase, Window, point,
};
use parking_lot::Mutex;

use crate::app::AppView;

static LAUNCHED: OnceLock<Instant> = OnceLock::new();

/// Whether the left button is down between commands, after `press` or `drag … hold`.
static PRESSED: AtomicBool = AtomicBool::new(false);

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
            let mut stall = STALL.lock();
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

/// Whether a script drives this run. Its window never shows, which macOS reports as hidden,
/// so the app follows the script's `hide` and `show` steps instead.
pub fn scripted() -> bool {
    std::env::var_os("GITBUTLER_GPUI_SCRIPT").is_some()
}

pub fn run_from_env(window: AnyWindowHandle, cx: &mut App) {
    let Some(path) = std::env::var_os("GITBUTLER_GPUI_SCRIPT").map(PathBuf::from) else {
        return;
    };
    // Read as lines arrive, so a pipe (`tail -f steps | …` with the script at /dev/stdin)
    // drives the app a step at a time. Opened there too, as a pipe's open waits for a writer.
    let (send, mut script) = futures::channel::mpsc::unbounded();
    std::thread::spawn(move || {
        use std::io::BufRead as _;
        let file = match std::fs::File::open(&path) {
            Ok(file) => file,
            Err(err) => return eprintln!("can't read {}: {err}", path.display()),
        };
        for line in std::io::BufReader::new(file).lines().map_while(Result::ok) {
            if send.unbounded_send(line).is_err() {
                break;
            }
        }
    });
    // Unseen, macOS naps the app after half a minute: its timers coalesce and its
    // threads lose priority, so every step slows down. Timings should be a visible window's.
    #[cfg(target_os = "macos")]
    std::mem::forget(
        objc2_foundation::NSProcessInfo::processInfo().beginActivityWithOptions_reason(
            objc2_foundation::NSActivityOptions::UserInitiated
                | objc2_foundation::NSActivityOptions::LatencyCritical,
            &objc2_foundation::NSString::from_str("Scripted run"),
        ),
    );
    watch_stalls(cx);
    // In the window, where a script sees and clicks them, not in a native alert, and in the
    // log, where it reads what the narrow dialog cuts off.
    cx.set_prompt_builder(|level, message, detail, actions, handle, window, cx| {
        eprintln!("PROMPT {message} | {}", detail.unwrap_or_default());
        gpui::fallback_prompt_renderer(level, message, detail, actions, handle, window, cx)
    });
    cx.spawn(async move |cx| {
        use futures::StreamExt as _;
        let mut line_no = 0;
        while let Some(line) = script.next().await {
            line_no += 1;
            // A comment starts a line or follows a space, so `type fix #123` keeps its `#`.
            let line = match line.trim_start().starts_with('#') {
                true => "",
                false => line.split(" #").next().unwrap_or_default().trim(),
            };
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
                    "hold" => {
                        let parts: Vec<&str> = rest.split_whitespace().collect();
                        let [label, key, times, every] = parts[..] else {
                            anyhow::bail!("hold takes a label, a key, how many times and every how many ms");
                        };
                        let (times, every): (usize, u64) = (times.parse()?, every.parse()?);
                        anyhow::ensure!(times > 0, "hold presses at least once");
                        *STALL.lock() =
                            (Some(Instant::now()), Duration::ZERO);
                        let mut took = Vec::with_capacity(times);
                        for _ in 0..times {
                            let start = Instant::now();
                            cx.update_window(window, |_, window, cx| {
                                dispatch("key", key, window, cx)
                            })??;
                            frames(window, cx).await?;
                            took.push(start.elapsed());
                            // The next press comes on time, or right away when this one drew late.
                            let left = Duration::from_millis(every).saturating_sub(start.elapsed());
                            cx.background_executor().timer(left).await;
                        }
                        let stall = STALL.lock().1;
                        let each: Vec<String> = (took.iter())
                            .map(|took| format!("{:.0}", took.as_secs_f64() * 1e3))
                            .collect();
                        took.sort();
                        let ms =
                            |at: usize| took.get(at).map_or(0., |took| took.as_secs_f64() * 1e3);
                        println!(
                            "HOLD {label} n={times} median={:.1}ms p90={:.1}ms max={:.1}ms stall={:.1}ms each={}",
                            ms(times / 2),
                            ms(times * 9 / 10),
                            ms(times - 1),
                            stall.as_secs_f64() * 1e3,
                            each.join(","),
                        );
                        Ok(())
                    }
                    "perf" => {
                        let (label, input) = rest.split_once(' ').unwrap_or((rest, ""));
                        // Draw first so the timing doesn't include catching up on earlier steps.
                        cx.update_window(window, |_, window, cx| frame(window, cx))?;
                        *STALL.lock() =
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
                        let stall = STALL.lock().1.max(draw);
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
                eprintln!("script line {line_no}: {line}: {err:#}");
            }
            // Let the frame settle before the next step, and draw what it asked to draw again
            // (a measured height), as macOS would for a window on screen.
            cx.background_executor()
                .timer(Duration::from_millis(120))
                .await;
            cx.update_window(window, |_, window, cx| frame(window, cx))
                .ok();
        }
        // No one sees the window, so it has nothing left to do once the script ends: at the
        // end of the file, or when the last writer closes the pipe.
        cx.update(|cx| cx.quit());
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
    // The window position the command takes.
    let at = || -> Result<Point<Pixels>> {
        let [x, y] = numbers()?[..] else {
            bail!("{command} takes x y");
        };
        Ok(point(Pixels::from(x), Pixels::from(y)))
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
            window.dispatch_keystroke(keystroke.clone(), cx);
            // Let go, which presses a focused button.
            window.dispatch_event(PlatformInput::KeyUp(KeyUpEvent { keystroke }), cx);
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
            let at = at()?;
            let pressed = PRESSED.load(Ordering::Relaxed).then_some(MouseButton::Left);
            window.dispatch_event(mouse_move(at, pressed, Modifiers::default()), cx);
        }
        "press" => {
            let at = at()?;
            window.dispatch_event(
                mouse_down(MouseButton::Left, at, Modifiers::default(), 1),
                cx,
            );
            PRESSED.store(true, Ordering::Relaxed);
        }
        "scroll" => {
            let (x, y, dy, dx) = match numbers()?[..] {
                [x, y, dy] => (x, y, dy, 0.),
                [x, y, dy, dx] => (x, y, dy, dx),
                _ => bail!("scroll takes x y dy, and dx to scroll sideways"),
            };
            window.dispatch_event(
                PlatformInput::ScrollWheel(ScrollWheelEvent {
                    position: point(Pixels::from(x), Pixels::from(y)),
                    delta: ScrollDelta::Pixels(point(Pixels::from(-dx), Pixels::from(-dy))),
                    modifiers: Modifiers::default(),
                    touch_phase: TouchPhase::Moved,
                }),
                cx,
            );
        }
        "click" | "rclick" | "dclick" | "tclick" => {
            let at = at()?;
            let button = if command == "rclick" {
                MouseButton::Right
            } else {
                MouseButton::Left
            };
            // A double or triple click is the clicks before it, then one with that count, a
            // frame apart, as a hand's are, so each sees what the one before it changed.
            let clicks = match command {
                "dclick" => 2,
                "tclick" => 3,
                _ => 1,
            };
            for count in 1..=clicks {
                if count > 1 {
                    frame(window, cx);
                }
                window.dispatch_event(mouse_move(at, None, modifiers), cx);
                window.dispatch_event(mouse_down(button, at, modifiers, count), cx);
                window.dispatch_event(mouse_up(button, at, modifiers, count), cx);
            }
        }
        "drag" => {
            let [x1, y1, x2, y2] = numbers()?[..] else {
                bail!("drag takes x1 y1 x2 y2");
            };
            let (from, to) = (
                point(Pixels::from(x1), Pixels::from(y1)),
                point(Pixels::from(x2), Pixels::from(y2)),
            );
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
            if held("hold") {
                PRESSED.store(true, Ordering::Relaxed);
            } else {
                window.dispatch_event(mouse_up(left, to, modifiers, 1), cx);
            }
        }
        "release" => {
            // Let go of a drag held by `drag … hold` where the pointer is, with no move first,
            // as a hand does.
            let at = at()?;
            window.dispatch_event(mouse_up(MouseButton::Left, at, Modifiers::default(), 1), cx);
            PRESSED.store(false, Ordering::Relaxed);
        }
        "drop" => {
            // Finish a drag left held by `drag … hold`, after frames have run.
            let (at, left) = (at()?, MouseButton::Left);
            window.dispatch_event(mouse_move(at, Some(left), Modifiers::default()), cx);
            window.dispatch_event(mouse_up(left, at, Modifiers::default(), 1), cx);
            PRESSED.store(false, Ordering::Relaxed);
        }
        // What the system's folder picker would hand Add a repository.
        "add" => {
            let app = window.root::<AppView>().flatten().context("no app view")?;
            app.update(cx, |app, cx| {
                app.store
                    .update(cx, |store, cx| store.add_project(PathBuf::from(rest), cx))
            });
        }
        // The window as if another app's covered it, or shown again.
        "hide" | "show" => {
            let app = window.root::<AppView>().flatten().context("no app view")?;
            app.update(cx, |app, cx| {
                (app.store).update(cx, |store, cx| store.set_visible(command == "show", cx))
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
            window.resize(gpui::size(Pixels::from(width), Pixels::from(height)));
        }
        "shot" => {
            // A path may have spaces, so a region is the last four words, when they're numbers.
            let (path, region): (&str, Option<[f32; 4]>) =
                match rest.rsplitn(5, ' ').collect::<Vec<_>>()[..] {
                    [height, width, y, x, path] => {
                        match (x.parse(), y.parse(), width.parse(), height.parse()) {
                            (Ok(x), Ok(y), Ok(width), Ok(height)) => {
                                (path, Some([x, y, width, height]))
                            }
                            _ => (rest, None),
                        }
                    }
                    _ => (rest, None),
                };
            let image = window.render_to_image()?;
            let scale = window.scale_factor();
            let image = match region {
                // At the window's own scale, so where something shows is where to click it.
                None => image::imageops::resize(
                    &image,
                    (image.width() as f32 / scale).round() as u32,
                    (image.height() as f32 / scale).round() as u32,
                    image::imageops::FilterType::Triangle,
                ),
                // A closer look, so every pixel the display has.
                Some([x, y, width, height]) => image::imageops::crop_imm(
                    &image,
                    (x * scale) as u32,
                    (y * scale) as u32,
                    (width * scale) as u32,
                    (height * scale) as u32,
                )
                .to_image(),
            };
            image.save(path).with_context(|| format!("saving {path}"))?;
        }
        // The key contexts from the window down to what's focused, by their names.
        "keys" => {
            let contexts: Vec<String> = (window.context_stack().iter())
                .filter_map(|context| Some(context.primary()?.key.to_string()))
                .collect();
            println!("KEYS {}", contexts.join(" > "));
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
