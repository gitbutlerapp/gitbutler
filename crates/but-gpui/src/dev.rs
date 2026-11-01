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
//!                         # after `press` or `drag … hold`, a drag's move; append `cmd`,
//!                         # `alt` or `shift` to hold them
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
//! pinch 900 400 0.5       # pinch a trackpad there, zooming in by half again, out below 0
//! add /path/to/repo       # add a project, as Add a repository does with the folder picked
//! keymap tui              # pick a keymap, gitbutler or tui, as Settings › Keyboard does
//! sh but commit -m x      # run a shell command, as an agent or another app would
//! size 900 500           # resize the window, in logical pixels
//! shot /tmp/view.png      # render the current frame to a PNG, a pixel per logical pixel, so
//!                         # a point read off it is one `click` takes
//! shot /tmp/a.png 0 0 400 300  # only that region, in logical pixels, at the display's scale
//! clock stop              # stop the clock motion is drawn at (`clock::now`), so folds and
//!                         # slides move only as `tick` and `film` move it; `clock run` runs
//!                         # it again from there
//! tick 8                  # move the stopped clock on 8ms
//! film /tmp/f 240 8       # draw a frame every 8ms of the stopped clock for 240ms, saving each
//!                         # as `shot` does, all of them labelled in a grid in sheet.png, and
//!                         # printing what changed in each from the frame before, in clusters
//!                         # of nearby pixels, largest first, as `FILM 003 t=24ms changed
//!                         # 2.1% in 2 places: 340x80 at 0,96 (5400px Δ255), 2x2 at 330,60
//!                         # (4px Δ30)`: size, place, pixels and the most one changed (0–255);
//!                         # a change in frame 000 is marked `JUMP`, and one after the film
//!                         # had held still `SNAP`, on its line and on the last
//! film /tmp/f 240 8 0 0 400 300  # only that region
//! film /tmp/f 240 8 -- click 306 66  # save before.png, run the input, then film it, on a
//!                         # clock stopped for the while
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
    MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, PinchEvent, Pixels, PlatformInput,
    Point, ScrollDelta, ScrollWheelEvent, TouchPhase, Window, point,
};
use image::RgbaImage;
use parking_lot::Mutex;

use crate::app::AppView;
use crate::clock;
use crate::keymap::Keymap;

static LAUNCHED: OnceLock<Instant> = OnceLock::new();

/// The most frames a `film` draws, so a slip in its numbers doesn't run for minutes.
const MOST_FRAMES: usize = 240;

/// How many of a frame's clusters of changed pixels `film` describes, the largest.
const CLUSTERS: usize = 3;

/// How close changed pixels are to be in one cluster, in a frame's pixels: close enough that a
/// word's letters make one, not one each.
const REACH: u32 = 2;

/// How many pixels a side a `film`'s sheet has at most, the frames shrunk to fit: an agent sees
/// no more of an image than that.
const SHEET_SIDE: u32 = 2000;

/// The pixels between a sheet's frames, and their colour.
const GAP: u32 = 4;
const GAP_COLOR: image::Rgba<u8> = image::Rgba([128, 128, 128, 255]);

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
                    "film" => film(rest, window, cx).await,
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

/// `film`: draw a frame every so often of the stopped clock, as a display would, and save each,
/// a sheet of them all, and what changed in each. Given an input after `--`, it saves the frame
/// before it, runs it, and films on a clock it stops for the while.
async fn film(rest: &str, window: AnyWindowHandle, cx: &mut gpui::AsyncApp) -> Result<()> {
    let (rest, input) = match rest.split_once(" -- ") {
        Some((rest, input)) => (rest, Some(input.trim())),
        None => (rest, None),
    };
    let (dir, length, every, region) = match (trailing_numbers(rest), trailing_numbers(rest)) {
        (Some((dir, [length, every, x, y, width, height])), _) => {
            (dir, length, every, Some([x, y, width, height]))
        }
        (None, Some((dir, [length, every]))) => (dir, length, every, None),
        (None, None) => bail!(
            "film takes a folder, how many ms and every how many, then a region, then -- and an input"
        ),
    };
    anyhow::ensure!(
        clock::stopped() || input.is_some(),
        "film moves a stopped clock: `clock stop` before the input it films, or give film the input after --"
    );
    anyhow::ensure!(
        every > 0.,
        "film draws a frame every so many ms, more than 0"
    );
    let tick = Duration::try_from_secs_f32(every / 1e3)?;
    let count = (length / every).max(0.) as usize + 1;
    anyhow::ensure!(
        count <= MOST_FRAMES,
        "film draws at most {MOST_FRAMES} frames"
    );
    let dir = PathBuf::from(dir);
    std::fs::create_dir_all(&dir).with_context(|| format!("creating {}", dir.display()))?;
    // Where a frame's pixels are in the window: a region's are at the display's scale.
    let scale = cx.update_window(window, |_, window, _| window.scale_factor())?;
    let (origin, scale) = match region {
        Some([x, y, _, _]) => (point(x, y), scale),
        None => (point(0., 0.), 1.),
    };
    let running = !clock::stopped();
    let filmed = async {
        let tiles = count + usize::from(input.is_some());
        let (mut sheet, mut last, mut last_change) = (None::<Sheet>, None::<RgbaImage>, None);
        // Whether frame 000 changed, before the clock moved, when the changes after the film
        // had held still came, and whether it holds still now after changing.
        let (mut jumped, mut snaps, mut held) = (false, Vec::new(), false);
        if let Some(input) = input {
            clock::stop();
            let image = cx.update_window(window, |_, window, cx| {
                frame(window, cx);
                capture(region, window)
            })??;
            save(&image, &dir.join("before.png"))?;
            println!("FILM before");
            (sheet.get_or_insert_with(|| Sheet::new(tiles, image.dimensions())))
                .add(0, &image, "before");
            last = Some(image);
            let (command, rest) = input.split_once(' ').unwrap_or((input, ""));
            cx.update_window(window, |_, window, cx| dispatch(command, rest, window, cx))??;
            cx.background_executor()
                .timer(Duration::from_millis(1))
                .await;
        }
        for n in 0..count {
            let image = cx.update_window(window, |_, window, cx| {
                if n > 0 {
                    clock::tick(tick);
                }
                frame(window, cx);
                capture(region, window)
            })??;
            save(&image, &dir.join(format!("{n:03}.png")))?;
            let at = n as f32 * every;
            let change = match last.as_ref().map(|last| changed(last, &image)) {
                None => String::new(),
                Some(None) => {
                    held = last_change.is_some();
                    " unchanged".to_owned()
                }
                Some(Some(change)) => {
                    let mark = match (n, held) {
                        (0, _) => {
                            jumped = true;
                            " JUMP"
                        }
                        (_, true) => {
                            snaps.push(at);
                            " SNAP"
                        }
                        (_, false) => "",
                    };
                    held = false;
                    last_change = Some(at);
                    format!("{mark} {}", describe(&change, origin, scale))
                }
            };
            println!("FILM {n:03} t={at:.0}ms{change}");
            (sheet.get_or_insert_with(|| Sheet::new(tiles, image.dimensions()))).add(
                tiles - count + n,
                &image,
                &format!("{at:.0}ms"),
            );
            last = Some(image);
            // Let what the frame started run, as the display's next beat would.
            cx.background_executor()
                .timer(Duration::from_millis(1))
                .await;
        }
        anyhow::Ok((sheet, last_change, jumped, snaps))
    }
    .await;
    if running {
        clock::run();
    }
    let (sheet, last_change, jumped, snaps) = filmed?;
    let Some(sheet) = sheet else {
        return Ok(());
    };
    save(&sheet.image, &dir.join("sheet.png"))?;
    let last_change = match last_change {
        Some(at) => format!("the last change at {at:.0}ms"),
        None => "nothing changed".to_owned(),
    };
    let before = match input {
        Some(_) => " and before.png",
        None => "",
    };
    // On this line too, so a read that keeps only it still sees them.
    let jump = match jumped {
        true => "; JUMP at 0ms",
        false => "",
    };
    let snap = match snaps.is_empty() {
        true => String::new(),
        false => {
            let at: Vec<String> = snaps.iter().map(|at| format!("{at:.0}ms")).collect();
            format!("; SNAP at {}", at.join(", "))
        }
    };
    println!(
        "FILM {count} frames{before} in {}, {last_change}{jump}{snap}; sheet.png has them {} to a row",
        dir.display(),
        sheet.columns,
    );
    Ok(())
}

fn save(image: &RgbaImage, path: &std::path::Path) -> Result<()> {
    (image.save(path)).with_context(|| format!("saving {}", path.display()))
}

/// What changed between two frames: how many of their pixels, of how many, and the clusters of
/// those near one another, the largest first.
struct Change {
    count: u64,
    total: u64,
    clusters: Vec<Cluster>,
}

/// Changed pixels within [`REACH`] of one another: how many, their least and greatest x and y, in the
/// frames' pixels, and how far the most changed of them did, from 0 to 255 in a channel.
struct Cluster {
    count: u64,
    low: (u32, u32),
    high: (u32, u32),
    delta: u8,
}

impl Cluster {
    fn new(x: u32, y: u32, delta: u8) -> Cluster {
        Cluster {
            count: 1,
            low: (x, y),
            high: (x, y),
            delta,
        }
    }

    fn add(&mut self, x: u32, y: u32, delta: u8) {
        self.count += 1;
        self.low = (self.low.0.min(x), self.low.1.min(y));
        self.high = (self.high.0.max(x), self.high.1.max(y));
        self.delta = self.delta.max(delta);
    }
}

fn changed(before: &RgbaImage, after: &RgbaImage) -> Option<Change> {
    let (width, height) = after.dimensions();
    let total = u64::from(width) * u64::from(height);
    if before.dimensions() != after.dimensions() {
        let mut all = Cluster::new(0, 0, u8::MAX);
        all.add(width.saturating_sub(1), height.saturating_sub(1), u8::MAX);
        all.count = total;
        return Some(Change {
            count: total,
            total,
            clusters: Vec::from([all]),
        });
    }
    // How far each pixel changed, in its most changed channel.
    let mut deltas = image::GrayImage::from_fn(width, height, |x, y| {
        let (was, now) = (before.get_pixel(x, y), after.get_pixel(x, y));
        let delta =
            (was.0.iter().zip(now.0)).fold(0, |most, (was, now)| most.max(was.abs_diff(now)));
        image::Luma([delta])
    });
    let (mut clusters, mut reached) = (Vec::new(), Vec::new());
    for y in 0..height {
        for x in 0..width {
            let Some(delta) = take(&mut deltas, x, y) else {
                continue;
            };
            let mut cluster = Cluster::new(x, y, delta);
            reached.push((x, y));
            while let Some((x, y)) = reached.pop() {
                for y in y.saturating_sub(REACH)..=y + REACH {
                    for x in x.saturating_sub(REACH)..=x + REACH {
                        if let Some(delta) = take(&mut deltas, x, y) {
                            cluster.add(x, y, delta);
                            reached.push((x, y));
                        }
                    }
                }
            }
            clusters.push(cluster);
        }
    }
    clusters.sort_by_key(|cluster| std::cmp::Reverse(cluster.count));
    (!clusters.is_empty()).then(|| Change {
        count: clusters.iter().map(|cluster| cluster.count).sum(),
        total,
        clusters,
    })
}

/// How far pixel `x`, `y` changed, if it did and no cluster has it yet; one has it after.
fn take(deltas: &mut image::GrayImage, x: u32, y: u32) -> Option<u8> {
    let image::Luma([delta]) = deltas.get_pixel_mut_checked(x, y)?;
    (*delta > 0).then(|| std::mem::take(delta))
}

/// A change as a FILM line says it: the share of the frame, then its largest clusters, each its
/// size at its place in the window, in points, then its pixels in the frame's and how far the
/// most changed of them did.
fn describe(change: &Change, origin: Point<f32>, scale: f32) -> String {
    let share = change.count as f32 / change.total as f32 * 100.;
    let share = match share < 0.1 {
        true => "<0.1%".to_owned(),
        false => format!("{share:.1}%"),
    };
    let mut clusters: Vec<String> = (change.clusters.iter().take(CLUSTERS))
        .map(|cluster| {
            let (x, y) = (cluster.low.0 as f32 / scale, cluster.low.1 as f32 / scale);
            let width = ((cluster.high.0 - cluster.low.0 + 1) as f32 / scale).max(1.);
            let height = ((cluster.high.1 - cluster.low.1 + 1) as f32 / scale).max(1.);
            format!(
                "{width:.0}x{height:.0} at {:.0},{:.0} ({}px Δ{})",
                origin.x + x,
                origin.y + y,
                cluster.count,
                cluster.delta,
            )
        })
        .collect();
    let rest = change.clusters.get(CLUSTERS..).unwrap_or_default();
    if !rest.is_empty() {
        clusters.push(format!(
            "and {} more ({}px Δ≤{})",
            rest.len(),
            rest.iter().map(|cluster| cluster.count).sum::<u64>(),
            rest.iter().fold(0, |most, cluster| most.max(cluster.delta)),
        ));
    }
    let places = match change.clusters.len() {
        1 => "1 place".to_owned(),
        n => format!("{n} places"),
    };
    format!("changed {share} in {places}: {}", clusters.join(", "))
}

/// A `film`'s frames in a grid, a row at a time, each labelled.
struct Sheet {
    image: RgbaImage,
    columns: u32,
    tile: (u32, u32),
}

/// A sheet's layout: how many frames a row, how much they shrink to, and how far from square
/// the sheet is.
struct Layout {
    columns: u32,
    fit: f32,
    lopsided: f32,
}

impl Sheet {
    /// Room for `count` frames `width` by `height`, in the columns that keep them largest within
    /// [`SHEET_SIDE`], and the squarest sheet of those.
    fn new(count: usize, (width, height): (u32, u32)) -> Sheet {
        let count = count.max(1) as u32;
        let (width, height) = (width.max(1), height.max(1));
        let best = (1..=count)
            .map(|columns| {
                let rows = count.div_ceil(columns);
                let across = SHEET_SIDE.saturating_sub((columns - 1) * GAP) as f32
                    / (columns * width) as f32;
                let down =
                    SHEET_SIDE.saturating_sub((rows - 1) * GAP) as f32 / (rows * height) as f32;
                let fit = across.min(down).min(1.);
                let lopsided = ((columns * width) as f32 / (rows * height) as f32)
                    .ln()
                    .abs();
                Layout {
                    columns,
                    fit,
                    lopsided,
                }
            })
            .max_by(|a, b| (a.fit.total_cmp(&b.fit)).then(b.lopsided.total_cmp(&a.lopsided)));
        let Layout { columns, fit, .. } = best.unwrap_or(Layout {
            columns: 1,
            fit: 1.,
            lopsided: 0.,
        });
        let rows = count.div_ceil(columns);
        let tile = (
            ((width as f32 * fit).round() as u32).max(1),
            ((height as f32 * fit).round() as u32).max(1),
        );
        Sheet {
            image: RgbaImage::from_pixel(
                columns * (tile.0 + GAP) - GAP,
                rows * (tile.1 + GAP) - GAP,
                GAP_COLOR,
            ),
            columns,
            tile,
        }
    }

    /// Put frame `n` in its place, `label` at its top left.
    fn add(&mut self, n: usize, frame: &RgbaImage, label: &str) {
        let n = n as u32;
        let (width, height) = self.tile;
        let mut tile =
            image::imageops::resize(frame, width, height, image::imageops::FilterType::Triangle);
        draw_label(&mut tile, label);
        let x = (n % self.columns) * (width + GAP);
        let y = (n / self.columns) * (height + GAP);
        image::imageops::replace(&mut self.image, &tile, x.into(), y.into());
    }
}

/// `label` in white on black at the top left of `tile`, in [`glyph`]'s font, scaled up to read
/// on the sheet.
fn draw_label(tile: &mut RgbaImage, label: &str) {
    const BLACK: image::Rgba<u8> = image::Rgba([0, 0, 0, 255]);
    const WHITE: image::Rgba<u8> = image::Rgba([255, 255, 255, 255]);
    // A font pixel's side, in the tile's.
    let size = (tile.width() / 100).clamp(2, 4);
    let advance = 6 * size;
    let chars = label.chars().count() as u32;
    fill(
        tile,
        (0, 0),
        (2 * size + chars * advance - size, 9 * size),
        BLACK,
    );
    for (at, c) in (0..).zip(label.chars()) {
        let Some(rows) = glyph(c) else {
            continue;
        };
        for (row, bits) in (0..).zip(rows) {
            for column in 0..5 {
                if bits & (0b10000 >> column) != 0 {
                    let x = size + at * advance + column * size;
                    fill(tile, (x, size + row * size), (size, size), WHITE);
                }
            }
        }
    }
}

/// Paint the rectangle at `at` of `size` in `color`, as much of it as is in `image`.
fn fill(image: &mut RgbaImage, at: (u32, u32), size: (u32, u32), color: image::Rgba<u8>) {
    for y in at.1..at.1 + size.1 {
        for x in at.0..at.0 + size.0 {
            if let Some(pixel) = image.get_pixel_mut_checked(x, y) {
                *pixel = color;
            }
        }
    }
}

/// A 5×7 pixel font with what a sheet's labels say: each glyph's rows, top first, its left
/// column the highest of five bits.
fn glyph(c: char) -> Option<[u8; 7]> {
    Some(match c {
        '0' => [
            0b01110, 0b10001, 0b10011, 0b10101, 0b11001, 0b10001, 0b01110,
        ],
        '1' => [
            0b00100, 0b01100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110,
        ],
        '2' => [
            0b01110, 0b10001, 0b00001, 0b00010, 0b00100, 0b01000, 0b11111,
        ],
        '3' => [
            0b11111, 0b00010, 0b00100, 0b00010, 0b00001, 0b10001, 0b01110,
        ],
        '4' => [
            0b00010, 0b00110, 0b01010, 0b10010, 0b11111, 0b00010, 0b00010,
        ],
        '5' => [
            0b11111, 0b10000, 0b11110, 0b00001, 0b00001, 0b10001, 0b01110,
        ],
        '6' => [
            0b00110, 0b01000, 0b10000, 0b11110, 0b10001, 0b10001, 0b01110,
        ],
        '7' => [
            0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b01000, 0b01000,
        ],
        '8' => [
            0b01110, 0b10001, 0b10001, 0b01110, 0b10001, 0b10001, 0b01110,
        ],
        '9' => [
            0b01110, 0b10001, 0b10001, 0b01111, 0b00001, 0b00010, 0b01100,
        ],
        'b' => [
            0b10000, 0b10000, 0b10110, 0b11001, 0b10001, 0b10001, 0b11110,
        ],
        'e' => [
            0b00000, 0b00000, 0b01110, 0b10001, 0b11111, 0b10000, 0b01110,
        ],
        'f' => [
            0b00110, 0b01001, 0b01000, 0b11100, 0b01000, 0b01000, 0b01000,
        ],
        'm' => [
            0b00000, 0b00000, 0b11010, 0b10101, 0b10101, 0b10101, 0b10101,
        ],
        'o' => [
            0b00000, 0b00000, 0b01110, 0b10001, 0b10001, 0b10001, 0b01110,
        ],
        'r' => [
            0b00000, 0b00000, 0b10110, 0b11001, 0b10000, 0b10000, 0b10000,
        ],
        's' => [
            0b00000, 0b00000, 0b01111, 0b10000, 0b01110, 0b00001, 0b11110,
        ],
        _ => return None,
    })
}

/// `rest` up to its last `N` words, and those, when they're all numbers: a path before them may
/// have spaces.
fn trailing_numbers<const N: usize>(rest: &str) -> Option<(&str, [f32; N])> {
    let mut numbers = [0.; N];
    let mut before = rest.trim_end();
    for number in numbers.iter_mut().rev() {
        let (head, word) = before.rsplit_once(' ')?;
        *number = word.parse().ok()?;
        before = head.trim_end();
    }
    Some((before, numbers))
}

/// The current frame as `shot` saves it: the window at its own scale, so where something shows
/// is where to click it, or a region of it at the display's, for a closer look.
fn capture(region: Option<[f32; 4]>, window: &mut Window) -> Result<RgbaImage> {
    let image = window.render_to_image()?;
    let scale = window.scale_factor();
    Ok(match region {
        None => image::imageops::resize(
            &image,
            (image.width() as f32 / scale).round() as u32,
            (image.height() as f32 / scale).round() as u32,
            image::imageops::FilterType::Triangle,
        ),
        Some([x, y, width, height]) => image::imageops::crop_imm(
            &image,
            (x * scale) as u32,
            (y * scale) as u32,
            (width * scale) as u32,
            (height * scale) as u32,
        )
        .to_image(),
    })
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
            window.dispatch_event(mouse_move(at, pressed, modifiers), cx);
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
        "pinch" => {
            let [x, y, delta] = numbers()?[..] else {
                bail!("pinch takes x y and how much it zooms by");
            };
            window.dispatch_event(
                PlatformInput::Pinch(PinchEvent {
                    position: point(Pixels::from(x), Pixels::from(y)),
                    delta,
                    modifiers: Modifiers::default(),
                    phase: TouchPhase::Moved,
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
        // The keymap Settings › Keyboard would pick, without opening it.
        "keymap" => {
            let keymap = (Keymap::ALL.into_iter())
                .find(|keymap| keymap.name().eq_ignore_ascii_case(rest))
                .with_context(|| {
                    let names = Keymap::ALL.map(|keymap| keymap.name().to_lowercase());
                    format!("keymap takes {}", names.join(" or "))
                })?;
            let app = window.root::<AppView>().flatten().context("no app view")?;
            app.update(cx, |app, cx| {
                (app.store).update(cx, |store, cx| store.set_prefs(|p| p.keymap = keymap, cx))
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
            // A region is the last four words, when they're numbers.
            let (path, region) = match trailing_numbers(rest) {
                Some((path, region)) => (path, Some(region)),
                None => (rest, None),
            };
            (capture(region, window)?)
                .save(path)
                .with_context(|| format!("saving {path}"))?;
        }
        "clock" => match rest {
            "stop" => clock::stop(),
            "run" => clock::run(),
            _ => bail!("clock takes stop or run"),
        },
        "tick" => {
            let by = rest.parse::<f32>().context("tick takes milliseconds")?;
            let by = Duration::try_from_secs_f32(by / 1e3).context("tick takes milliseconds")?;
            anyhow::ensure!(clock::tick(by), "the clock runs: `clock stop` first");
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
