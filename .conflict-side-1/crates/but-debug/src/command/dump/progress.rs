use std::{
    io,
    sync::{Arc, atomic::Ordering},
    time::Duration,
};

use anyhow::{Result, bail};

/// Owns dump progress counters, rendering, and abort state.
pub(super) struct DumpProgress {
    /// Keeps the shared progress tree alive for the renderer and task items.
    _root: Arc<prodash::tree::Root>,
    /// Background line renderer for terminal progress output.
    /// It terminates automatically once `_root` is dropped.
    renderer: Option<prodash::render::line::JoinHandle>,
    /// Parent task grouping all dump counters as a headline.
    _task: prodash::tree::Item,
    /// Counter for bytes read from repository files.
    bytes_read: prodash::tree::Item,
    /// Counter for bytes written to the archive file.
    bytes_written: prodash::tree::Item,
    /// Counter for regular files and symlinks added to the archive.
    files_processed: prodash::tree::Item,
    /// Signal handler registration scoped to this dump operation.
    interrupt: Option<gix::interrupt::Deregister>,
}

impl DumpProgress {
    pub(super) fn new() -> Result<Self> {
        let root: Arc<prodash::tree::Root> = prodash::tree::root::Options {
            message_buffer_capacity: 200,
            ..Default::default()
        }
        .into();
        let renderer = prodash::render::line(
            std::io::stderr(),
            Arc::downgrade(&root),
            prodash::render::line::Options {
                level_filter: None,
                frames_per_second: 6.0,
                initial_delay: Some(Duration::from_secs(1)),
                timestamp: true,
                throughput: true,
                hide_cursor: true,
                ..Default::default()
            }
            .auto_configure(prodash::render::line::StreamKind::Stderr),
        );

        let mut task = root.add_child("dump repository");
        let throughput = prodash::unit::display::Mode::with_throughput();
        let bytes_read = task.add_child("bytes in");
        bytes_read.init(
            None,
            Some(prodash::unit::dynamic_and_mode(
                prodash::unit::Bytes,
                throughput,
            )),
        );

        let files_processed = task.add_child("processed");
        files_processed.init(
            None,
            Some(prodash::unit::label_and_mode("files", throughput)),
        );

        let bytes_written = task.add_child("archive out");
        bytes_written.init(
            None,
            Some(prodash::unit::dynamic_and_mode(
                prodash::unit::Bytes,
                throughput,
            )),
        );

        gix::interrupt::reset();
        #[allow(unsafe_code)]
        let interrupt = unsafe {
            // SAFETY: The callback runs from a signal handler, so it must only
            // perform signal-safe work. It is intentionally empty; gix updates
            // its global atomic interrupt flag after invoking it.
            gix::interrupt::init_handler(1, || {})
        }?;

        Ok(Self {
            _root: root,
            renderer: Some(renderer),
            _task: task,
            bytes_read,
            bytes_written,
            files_processed,
            interrupt: Some(interrupt),
        })
    }

    pub(super) fn set_input_upper_bounds(&self, bytes_read: usize, files_processed: usize) {
        self.bytes_read.set_max(Some(bytes_read));
        self.files_processed.set_max(Some(files_processed));
    }

    fn add_bytes_read(&self, amount: usize) {
        self.bytes_read.inc_by(amount);
    }

    fn add_bytes_written(&self, amount: usize) {
        self.bytes_written.inc_by(amount);
    }

    pub(super) fn add_file_processed(&self) {
        self.files_processed.inc();
    }

    pub(super) fn check_abort(&self) -> Result<()> {
        if gix::interrupt::IS_INTERRUPTED.load(Ordering::SeqCst) {
            bail!("Interrupted while creating repository dump");
        }
        Ok(())
    }

    fn check_abort_io(&self) -> io::Result<()> {
        if gix::interrupt::IS_INTERRUPTED.load(Ordering::SeqCst) {
            return Err(io::Error::other(
                "interrupted while creating repository dump",
            ));
        }
        Ok(())
    }
}

impl Drop for DumpProgress {
    fn drop(&mut self) {
        if let Some(renderer) = self.renderer.take() {
            renderer.shutdown_and_wait();
        }
        if let Some(interrupt) = self.interrupt.take() {
            interrupt.with_reset(true).deregister().ok();
        }
    }
}

/// Reader wrapper that counts source bytes and observes abort requests.
pub(super) struct ProgressReader<'progress, R> {
    inner: R,
    progress: &'progress DumpProgress,
}

impl<'progress, R> ProgressReader<'progress, R> {
    pub(super) fn new(inner: R, progress: &'progress DumpProgress) -> Self {
        Self { inner, progress }
    }
}

impl<R: io::Read> io::Read for ProgressReader<'_, R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.progress.check_abort_io()?;
        let amount = self.inner.read(buf)?;
        self.progress.add_bytes_read(amount);
        Ok(amount)
    }
}

/// Writer wrapper that counts archive bytes and observes abort requests.
pub(super) struct ProgressWriter<'progress, W> {
    inner: W,
    progress: &'progress DumpProgress,
}

impl<'progress, W> ProgressWriter<'progress, W> {
    pub(super) fn new(inner: W, progress: &'progress DumpProgress) -> Self {
        Self { inner, progress }
    }

    pub(super) fn into_inner(self) -> W {
        self.inner
    }
}

impl<W: io::Write> io::Write for ProgressWriter<'_, W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.progress.check_abort_io()?;
        let amount = self.inner.write(buf)?;
        self.progress.add_bytes_written(amount);
        Ok(amount)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.progress.check_abort_io()?;
        self.inner.flush()
    }
}

impl<W: io::Seek> io::Seek for ProgressWriter<'_, W> {
    fn seek(&mut self, pos: io::SeekFrom) -> io::Result<u64> {
        self.progress.check_abort_io()?;
        self.inner.seek(pos)
    }
}
