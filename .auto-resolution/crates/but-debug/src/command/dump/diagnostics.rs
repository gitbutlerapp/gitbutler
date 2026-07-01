//! Diagnostics archive support for `but-debug dump`.

use std::{
    fs,
    io::{self, Write as _},
    path::Path,
    process::{Command, Stdio},
    sync::atomic::{AtomicUsize, Ordering},
    time::{Duration, Instant},
};

use anyhow::{Context as _, Result};
use zip::{CompressionMethod, ZipWriter, write::SimpleFileOptions};

use crate::{
    args::{Args, DiagnosticsDumpArgs},
    command::dump::default_output_path,
};

/// Capture diagnostics by discovering a GitButler context at `current_dir`.
pub(super) fn capture_from_current_dir(
    current_dir: &Path,
    dot_timeout: Option<Duration>,
    out: &mut dyn io::Write,
    err: &mut dyn io::Write,
) -> Result<Diagnostics> {
    let ctx = but_ctx::Context::discover(current_dir).with_context(|| {
        format!(
            "Could not open GitButler context at '{}'",
            current_dir.display()
        )
    })?;
    Diagnostics::capture(&ctx, dot_timeout, out, err)
}

/// Execute the `dump diagnostics` subcommand.
pub(super) fn run(
    args: &Args,
    diagnostics_args: &DiagnosticsDumpArgs,
    out: &mut dyn io::Write,
    err: &mut dyn io::Write,
) -> Result<()> {
    let current_dir = super::effective_current_dir(args)?;
    let ctx = but_ctx::Context::discover(&current_dir).with_context(|| {
        format!(
            "Could not open GitButler context at '{}'",
            current_dir.display()
        )
    })?;
    let repo = ctx.repo.get()?;
    let output_path = match &diagnostics_args.archive.output {
        Some(path) => current_dir.join(path),
        None => default_output_path(&repo, "diagnostics")?,
    };
    let diagnostics = Diagnostics::capture(
        &ctx,
        dot_timeout(diagnostics_args.diagnostics.dot_timeout_seconds),
        out,
        err,
    )?;
    let archive_root = format!("{}-diagnostics", super::archive_base_name(&repo)?);
    let lock = super::acquire_archive_lock(&output_path, repo.workdir().map(Path::to_owned))?;
    let mut archive = ZipWriter::new(lock);
    diagnostics.write_to_archive(&mut archive, &archive_root)?;
    let lock = archive.finish()?;
    super::persist_archive(lock)?;

    writeln!(out, "Archive at: {}", output_path.display())?;
    super::open_archive_dir_unless_requested(
        &output_path,
        diagnostics_args.archive.no_open_archive_directory,
    )?;
    Ok(())
}

/// In-memory files produced for a diagnostics archive.
pub(super) struct Diagnostics {
    files: Vec<DiagnosticFile>,
}

impl Diagnostics {
    /// Capture graph and workspace diagnostics through a GitButler context.
    pub(super) fn capture(
        ctx: &but_ctx::Context,
        dot_timeout: Option<Duration>,
        out: &mut dyn io::Write,
        err: &mut dyn io::Write,
    ) -> Result<Self> {
        let (_guard, _repo, ws, _db) = ctx.workspace_and_db()?;
        let dot_graph = ws.graph.dot_graph_pruned();
        let workspace_debug = format!("{ws:#?}\n");

        let mut files = vec![
            DiagnosticFile::new("graph.dot", dot_graph.as_bytes().to_vec()),
            DiagnosticFile::new("workspace.ron.txt", workspace_debug.into_bytes()),
        ];
        if let Some(svg) = render_svg_with_timeout(&dot_graph, dot_timeout, out, err)? {
            files.push(DiagnosticFile::new("graph.svg", svg));
        }
        Ok(Self { files })
    }

    /// Write all diagnostics into `archive` below `archive_root`.
    pub(super) fn write_to_archive<W: io::Write + io::Seek>(
        &self,
        archive: &mut ZipWriter<W>,
        archive_root: &str,
    ) -> Result<()> {
        archive.add_directory(archive_root, directory_options())?;
        for file in &self.files {
            archive.start_file(
                format!("{archive_root}/{}", file.relative_path),
                file_options(),
            )?;
            archive.write_all(&file.contents)?;
        }
        Ok(())
    }

    /// Iterate diagnostics as relative archive paths and complete file contents.
    pub(super) fn entries(&self) -> impl Iterator<Item = (&'static str, &[u8])> + '_ {
        self.files
            .iter()
            .map(|file| (file.relative_path, file.contents.as_slice()))
    }

    /// Return the number of files that will be emitted.
    pub(super) fn file_count(&self) -> usize {
        self.files.len()
    }
}

pub(super) fn dot_timeout(seconds: u32) -> Option<Duration> {
    (seconds != 0).then(|| Duration::from_secs(u64::from(seconds)))
}

struct DiagnosticFile {
    /// Path below the diagnostics archive root, using zip `/` separators.
    relative_path: &'static str,
    /// Complete bytes written to the archive entry at `relative_path`.
    contents: Vec<u8>,
}

impl DiagnosticFile {
    fn new(relative_path: &'static str, contents: Vec<u8>) -> Self {
        Self {
            relative_path,
            contents,
        }
    }
}

/// Render `dot_graph` with Graphviz and return the generated SVG bytes.
///
/// Returns `Ok(Some(svg))` when `dot -Tsvg` exits successfully and the SVG file
/// can be read. Returns `Ok(None)` when `dot` is not installed, exits with a
/// non-zero status, or does not finish before `timeout`. A `None` timeout waits
/// indefinitely. Returns `Err` for
/// failures managing the temporary files or launching/polling the process.
fn render_svg_with_timeout(
    dot_graph: &str,
    timeout: Option<Duration>,
    out: &mut dyn io::Write,
    err: &mut dyn io::Write,
) -> Result<Option<Vec<u8>>> {
    static SUFFIX: AtomicUsize = AtomicUsize::new(0);

    let suffix = SUFFIX.fetch_add(1, Ordering::SeqCst);
    let dir = std::env::temp_dir().join(format!(
        "but-debug-diagnostics-{}-{suffix}",
        std::process::id()
    ));
    fs::create_dir(&dir)
        .with_context(|| format!("Could not create temporary directory '{}'", dir.display()))?;
    let result = render_svg_with_timeout_in(dot_graph, timeout, &dir, out, err);
    let cleanup = fs::remove_dir_all(&dir);
    match (result, cleanup) {
        (Ok(value), Ok(())) => Ok(value),
        (Err(err), _) => Err(err),
        (Ok(_), Err(err)) => Err(err)
            .with_context(|| format!("Could not remove temporary directory '{}'", dir.display())),
    }
}

fn render_svg_with_timeout_in(
    dot_graph: &str,
    timeout: Option<Duration>,
    dir: &Path,
    out: &mut dyn io::Write,
    err: &mut dyn io::Write,
) -> Result<Option<Vec<u8>>> {
    let dot_path = dir.join("graph.dot");
    let svg_path = dir.join("graph.svg");
    fs::write(&dot_path, dot_graph).with_context(|| {
        format!(
            "Could not write temporary dot file '{}'",
            dot_path.display()
        )
    })?;

    writeln!(
        out,
        "Running: dot -Tsvg {} -o {}",
        dot_path.display(),
        svg_path.display()
    )?;

    let mut child = match Command::new("dot")
        .arg("-Tsvg")
        .arg(&dot_path)
        .arg("-o")
        .arg(&svg_path)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
    {
        Ok(child) => child,
        Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(err) => return Err(err).context("Could not start dot to render graph SVG"),
    };

    let started_at = Instant::now();
    loop {
        if let Some(status) = child.try_wait()? {
            return if status.success() {
                fs::read(&svg_path)
                    .with_context(|| {
                        format!("Could not read generated SVG '{}'", svg_path.display())
                    })
                    .map(Some)
            } else {
                Ok(None)
            };
        }
        if let Some(timeout) = timeout
            && started_at.elapsed() >= timeout
        {
            writeln!(
                err,
                "dot did not finish within {timeout:?}; killing process {}",
                child.id()
            )?;
            child.kill().ok();
            child.wait().ok();
            return Ok(None);
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

fn file_options() -> SimpleFileOptions {
    SimpleFileOptions::default()
        .compression_method(CompressionMethod::Bzip2)
        .unix_permissions(0o644)
}

fn directory_options() -> SimpleFileOptions {
    SimpleFileOptions::default()
        .compression_method(CompressionMethod::Bzip2)
        .unix_permissions(0o755)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zero_dot_timeout_disables_timeout() {
        assert_eq!(dot_timeout(0), None, "zero seconds means no timeout");
        assert_eq!(
            dot_timeout(7),
            Some(Duration::from_secs(7)),
            "non-zero values are interpreted as seconds"
        );
    }
}
