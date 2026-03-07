use std::{io, sync::Arc, thread, time::Duration};

use anyhow::{Result, bail};

const DEFAULT_FRAME_RATE: f32 = 60.0;

pub fn long_running(duration: Duration, trace: bool) -> Result<()> {
    #[allow(unsafe_code)]
    let _interrupt_handler = unsafe {
        // SAFETY: The closure only calls a function that in turn sets an atomic. No memory handling is triggered.
        gix::interrupt::init_handler(1, gix::interrupt::trigger)?.auto_deregister()
    };

    let progress = progress_tree(trace);
    let renderer = setup_line_renderer(&progress);
    let result = thread::scope(|scope| -> Result<Option<usize>> {
        let rx = but_api::poc::long_running_non_blocking_scoped_thread(
            scope,
            duration,
            progress.add_child("long-running"),
            &gix::interrupt::IS_INTERRUPTED,
        );
        let mut last = None;
        for data in rx {
            last = Some(data?.0);
        }
        Ok(last)
    });
    renderer.shutdown_and_wait();

    let last = result?;
    if gix::interrupt::is_triggered() {
        bail!("Interrupted");
    }
    if let Some(last) = last {
        println!("Completed {last} step(s).");
    }
    Ok(())
}

fn progress_tree(trace: bool) -> Arc<prodash::tree::Root> {
    prodash::tree::root::Options {
        message_buffer_capacity: if trace { 10_000 } else { 200 },
        ..Default::default()
    }
    .into()
}

fn setup_line_renderer(progress: &Arc<prodash::tree::Root>) -> prodash::render::line::JoinHandle {
    prodash::render::line(
        io::stderr(),
        Arc::downgrade(progress),
        prodash::render::line::Options {
            frames_per_second: DEFAULT_FRAME_RATE,
            initial_delay: Some(Duration::from_millis(500)),
            timestamp: true,
            throughput: true,
            hide_cursor: true,
            ..prodash::render::line::Options::default()
        }
        .auto_configure(prodash::render::line::StreamKind::Stderr),
    )
}
