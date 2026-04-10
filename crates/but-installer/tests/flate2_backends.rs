#![cfg(unix)]

use std::{
    collections::BTreeMap,
    fs::File,
    io::Write,
    path::{Path, PathBuf},
    process::Command,
};

use flate2::{Compression, write::GzEncoder};
use tar::{Builder, Header};
use tempfile::tempdir;

const ITERATIONS: usize = 6;
const MAX_ACCEPTABLE_SLOWDOWN: f64 = 2.5;
const GUARDED_CASES: &[&str] = &["gzip_bufread_copy", "tar_archive_entries_bufread"];
const REPORT_ONLY_CASES: &[&str] = &[
    "gzip_read_copy",
    "gzip_read_small_chunks",
    "gzip_decompress_small_output",
    "tar_archive_entries",
];

#[derive(Debug, Clone)]
struct BenchSummary {
    backend: String,
    median_ns: u128,
    mean_ns: f64,
    output_bytes: u64,
}

#[test]
#[ignore = "run with cargo test -p but-installer --release --test flate2_backends -- --ignored --nocapture"]
fn buffered_gzip_workloads_stay_close_to_zlib_ng_baseline() {
    if cfg!(debug_assertions) {
        panic!("run this benchmark test in release mode");
    }

    let temp_dir = tempdir().unwrap();
    let fixture = temp_dir.path().join("gitbutler-installer-fixture.tar.gz");
    create_fixture_archive(&fixture).unwrap();

    let zlib_ng_csv = temp_dir.path().join("zlib-ng.csv");
    let zlib_rs_csv = temp_dir.path().join("zlib-rs.csv");

    run_backend_benchmark("zlib-ng-backend", &fixture, &zlib_ng_csv).unwrap();
    run_backend_benchmark("zlib-rs-backend", &fixture, &zlib_rs_csv).unwrap();

    let zlib_ng = parse_csv(&zlib_ng_csv).unwrap();
    let zlib_rs = parse_csv(&zlib_rs_csv).unwrap();

    for case in REPORT_ONLY_CASES
        .iter()
        .chain(GUARDED_CASES.iter())
        .copied()
    {
        let baseline = zlib_ng
            .get(case)
            .unwrap_or_else(|| panic!("missing zlib-ng case {case}"));
        let candidate = zlib_rs
            .get(case)
            .unwrap_or_else(|| panic!("missing zlib-rs case {case}"));
        let slowdown = candidate.median_ns as f64 / baseline.median_ns as f64;

        eprintln!(
            "{case}: {} median {:.2} ms / mean {:.2} ms, {} median {:.2} ms / mean {:.2} ms, slowdown {:.2}x",
            baseline.backend,
            nanos_to_ms(baseline.median_ns),
            nanos_to_ms_f64(baseline.mean_ns),
            candidate.backend,
            nanos_to_ms(candidate.median_ns),
            nanos_to_ms_f64(candidate.mean_ns),
            slowdown
        );

        assert_eq!(
            baseline.output_bytes, candidate.output_bytes,
            "benchmark case {case} decompressed different output sizes"
        );
    }

    for case in GUARDED_CASES {
        let baseline = zlib_ng
            .get(*case)
            .unwrap_or_else(|| panic!("missing zlib-ng case {case}"));
        let candidate = zlib_rs
            .get(*case)
            .unwrap_or_else(|| panic!("missing zlib-rs case {case}"));
        let slowdown = candidate.median_ns as f64 / baseline.median_ns as f64;

        assert!(
            slowdown <= MAX_ACCEPTABLE_SLOWDOWN,
            "{case} regressed beyond the allowed {:.1}x slowdown: {} {:.2} ms vs {} {:.2} ms ({:.2}x). See {} and {} for raw CSV output.",
            MAX_ACCEPTABLE_SLOWDOWN,
            baseline.backend,
            nanos_to_ms(baseline.median_ns),
            candidate.backend,
            nanos_to_ms(candidate.median_ns),
            slowdown,
            zlib_ng_csv.display(),
            zlib_rs_csv.display(),
        );
    }
}

fn create_fixture_archive(path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let file = File::create(path)?;
    let encoder = GzEncoder::new(file, Compression::default());
    let mut archive = Builder::new(encoder);

    append_bytes(
        &mut archive,
        "GitButler.app/Contents/MacOS/gitbutler-tauri",
        &synthetic_file_bytes(0, 256 * 1024),
    )?;
    append_bytes(
        &mut archive,
        "GitButler.app/Contents/MacOS/gitbutler-git-askpass",
        &synthetic_file_bytes(1, 96 * 1024),
    )?;

    for index in 0..96 {
        let path = format!("GitButler.app/Contents/Resources/assets/fixture-{index:03}.dat");
        append_bytes(
            &mut archive,
            &path,
            &synthetic_file_bytes(index + 2, 512 * 1024),
        )?;
    }

    archive.finish()?;
    let encoder = archive.into_inner()?;
    let mut file = encoder.finish()?;
    file.flush()?;
    Ok(())
}

fn append_bytes(
    archive: &mut Builder<GzEncoder<File>>,
    path: &str,
    contents: &[u8],
) -> Result<(), Box<dyn std::error::Error>> {
    let mut header = Header::new_gnu();
    header.set_path(path)?;
    header.set_mode(0o644);
    header.set_size(contents.len() as u64);
    header.set_cksum();
    archive.append(&header, contents)?;
    Ok(())
}

fn synthetic_file_bytes(seed: usize, len: usize) -> Vec<u8> {
    let mut data = Vec::with_capacity(len);
    let mut state = seed as u64 ^ 0x9E37_79B9_7F4A_7C15;
    let mut line = 0usize;

    while data.len() < len {
        state = state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);

        let record = format!(
            "fixture={seed:03} line={line:06} state={state:016x} gitbutler flate2 regression benchmark\n"
        );
        data.extend_from_slice(record.as_bytes());
        data.extend(std::iter::repeat_n(b'A' + (seed % 26) as u8, 96));
        data.extend_from_slice(&state.to_le_bytes());
        line += 1;
    }

    data.truncate(len);
    data
}

fn run_backend_benchmark(
    feature: &str,
    fixture: &Path,
    csv: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let manifest = bench_manifest();
    let output = Command::new("cargo")
        .args([
            "run",
            "--quiet",
            "--manifest-path",
            manifest.to_str().unwrap(),
            "--release",
            "--no-default-features",
            "--features",
            feature,
            "--",
            "--input",
            fixture.to_str().unwrap(),
            "--iterations",
            &ITERATIONS.to_string(),
            "--csv",
            csv.to_str().unwrap(),
        ])
        .output()?;

    if !output.status.success() {
        return Err(format!(
            "benchmark command failed for {feature}:\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        )
        .into());
    }

    Ok(())
}

fn bench_manifest() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("perf")
        .join("flate2-bench")
        .join("Cargo.toml")
}

fn parse_csv(path: &Path) -> Result<BTreeMap<String, BenchSummary>, Box<dyn std::error::Error>> {
    let content = std::fs::read_to_string(path)?;
    let mut by_case = BTreeMap::<String, Vec<(String, u128, u64)>>::new();

    for line in content.lines().skip(1) {
        if line.trim().is_empty() {
            continue;
        }

        let mut fields = line.split(',');
        let backend = fields.next().ok_or("missing backend")?.to_owned();
        let case = fields.next().ok_or("missing case")?.to_owned();
        let _iteration = fields.next().ok_or("missing iteration")?;
        let elapsed_ns: u128 = fields.next().ok_or("missing elapsed_ns")?.parse()?;
        let output_bytes: u64 = fields.next().ok_or("missing output_bytes")?.parse()?;
        by_case
            .entry(case)
            .or_default()
            .push((backend, elapsed_ns, output_bytes));
    }

    let mut summaries = BTreeMap::new();
    for (case, rows) in by_case {
        let backend = rows.first().ok_or("empty benchmark case")?.0.clone();
        let output_bytes = rows.first().ok_or("empty benchmark case")?.2;
        let mut timings: Vec<u128> = rows.iter().map(|(_, elapsed_ns, _)| *elapsed_ns).collect();
        timings.sort_unstable();
        let mean_ns = timings.iter().copied().sum::<u128>() as f64 / timings.len() as f64;
        let median_ns = timings[timings.len() / 2];

        summaries.insert(
            case,
            BenchSummary {
                backend,
                median_ns,
                mean_ns,
                output_bytes,
            },
        );
    }

    Ok(summaries)
}

fn nanos_to_ms(nanos: u128) -> f64 {
    nanos as f64 / 1_000_000.0
}

fn nanos_to_ms_f64(nanos: f64) -> f64 {
    nanos / 1_000_000.0
}
