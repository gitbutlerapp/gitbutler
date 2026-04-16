use std::collections::BTreeMap;
use std::env;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::hint::black_box;
use std::io::{Read, Write};
use std::mem::MaybeUninit;
use std::path::PathBuf;
use std::time::Instant;

use anyhow::{Context, Result, bail, ensure};
use flate2::bufread::ZlibDecoder;
use flate2::write::ZlibEncoder;
use flate2::{
    Compress, Compression, Decompress, FlushCompress, FlushDecompress, Status,
};
use serde::Serialize;

const CHUNK_IN: usize = 2_048;
const LARGE_CHUNK_OUT: usize = 2 * 1024 * 1024;
const VEC_RESERVE: usize = LARGE_CHUNK_OUT;
const PLAIN_LEN: usize = 4 * 1024 * 1024;
const CURRENT_SAMPLES: usize = 7;
const LEGACY_SAMPLES: usize = 7;
const FLATE2_VERSION: &str = "1.1.9";

fn main() -> Result<()> {
    let args = Args::parse()?;
    let fixture = Fixture::new()?;
    let report = match args.suite {
        Suite::Legacy => run_legacy_suite(&fixture, args.samples.unwrap_or(LEGACY_SAMPLES))?,
        Suite::Current => run_current_suite(&fixture, args.samples.unwrap_or(CURRENT_SAMPLES))?,
    };

    if let Some(path) = args.baseline {
        validate_against_baseline(&report, &path)?;
    }

    serde_json::to_writer_pretty(std::io::stdout().lock(), &report)?;
    println!();
    Ok(())
}

#[derive(Clone, Copy)]
enum Suite {
    Legacy,
    Current,
}

struct Args {
    suite: Suite,
    baseline: Option<PathBuf>,
    samples: Option<usize>,
}

impl Args {
    fn parse() -> Result<Self> {
        let mut suite = Suite::Current;
        let mut baseline = None;
        let mut samples = None;

        let mut args = env::args().skip(1);
        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--suite" => {
                    let value = args.next().context("missing value after --suite")?;
                    suite = match value.as_str() {
                        "legacy" => Suite::Legacy,
                        "current" => Suite::Current,
                        _ => bail!("unsupported suite '{value}'"),
                    };
                }
                "--baseline" => {
                    baseline = Some(PathBuf::from(
                        args.next().context("missing value after --baseline")?,
                    ));
                }
                "--samples" => {
                    let value = args.next().context("missing value after --samples")?;
                    let parsed = value.parse::<usize>().context("invalid --samples value")?;
                    ensure!(parsed >= 3, "--samples must be at least 3");
                    samples = Some(parsed);
                }
                "--help" | "-h" => {
                    println!(
                        "Usage: cargo run --release -p but-installer --bin flate2-backend-benchmark -- [--suite current|legacy] [--baseline path] [--samples n]"
                    );
                    std::process::exit(0);
                }
                _ => bail!("unsupported argument '{arg}'"),
            }
        }

        Ok(Self {
            suite,
            baseline,
            samples,
        })
    }
}

struct Fixture {
    plain: Vec<u8>,
    zlib: Vec<u8>,
    plain_hash: u64,
    zlib_hash: u64,
}

impl Fixture {
    fn new() -> Result<Self> {
        let plain = make_plain_data();
        let zlib = compress_fixture(&plain)?;
        let plain_hash = hash_bytes(&plain);
        let zlib_hash = hash_bytes(&zlib);
        Ok(Self {
            plain,
            zlib,
            plain_hash,
            zlib_hash,
        })
    }
}

#[derive(Serialize)]
struct BenchmarkReport {
    schema_version: u32,
    suite: &'static str,
    backend: &'static str,
    flate2_version: &'static str,
    plain_len: usize,
    zlib_len: usize,
    chunk_in: usize,
    large_chunk_out: usize,
    samples: usize,
    metrics: BTreeMap<String, Metric>,
}

#[derive(Serialize)]
struct Metric {
    median_nanos: u64,
    ns_per_byte: f64,
    min_nanos: u64,
    max_nanos: u64,
    checksum: u64,
}

fn run_legacy_suite(fixture: &Fixture, samples: usize) -> Result<BenchmarkReport> {
    let mut metrics = BTreeMap::new();
    metrics.insert(
        "chunked_decompress_with_large_output_buf".into(),
        bench_case(samples, fixture.plain.len(), || {
            chunked_decompress(fixture, DecompressMode::InitializedLargeOutput)
        })?,
    );

    Ok(BenchmarkReport {
        schema_version: 1,
        suite: "legacy",
        backend: backend_name(),
        flate2_version: FLATE2_VERSION,
        plain_len: fixture.plain.len(),
        zlib_len: fixture.zlib.len(),
        chunk_in: CHUNK_IN,
        large_chunk_out: LARGE_CHUNK_OUT,
        samples,
        metrics,
    })
}

fn run_current_suite(fixture: &Fixture, samples: usize) -> Result<BenchmarkReport> {
    let mut metrics = BTreeMap::new();
    metrics.insert(
        "chunked_compress_uninit_with_large_output_buf".into(),
        bench_case(samples, fixture.plain.len(), || {
            chunked_compress(fixture, CompressMode::UninitLargeOutput)
        })?,
    );
    metrics.insert(
        "chunked_compress_vec_with_reserved_spare_capacity".into(),
        bench_case(samples, fixture.plain.len(), || {
            chunked_compress(fixture, CompressMode::VecSpareCapacity)
        })?,
    );
    metrics.insert(
        "chunked_compress_with_large_output_buf".into(),
        bench_case(samples, fixture.plain.len(), || {
            chunked_compress(fixture, CompressMode::InitializedLargeOutput)
        })?,
    );
    metrics.insert(
        "chunked_decompress_uninit_with_large_output_buf".into(),
        bench_case(samples, fixture.plain.len(), || {
            chunked_decompress(fixture, DecompressMode::UninitLargeOutput)
        })?,
    );
    metrics.insert(
        "chunked_decompress_vec_with_reserved_spare_capacity".into(),
        bench_case(samples, fixture.plain.len(), || {
            chunked_decompress(fixture, DecompressMode::VecSpareCapacity)
        })?,
    );
    metrics.insert(
        "chunked_decompress_with_large_output_buf".into(),
        bench_case(samples, fixture.plain.len(), || {
            chunked_decompress(fixture, DecompressMode::InitializedLargeOutput)
        })?,
    );

    Ok(BenchmarkReport {
        schema_version: 1,
        suite: "current",
        backend: backend_name(),
        flate2_version: FLATE2_VERSION,
        plain_len: fixture.plain.len(),
        zlib_len: fixture.zlib.len(),
        chunk_in: CHUNK_IN,
        large_chunk_out: LARGE_CHUNK_OUT,
        samples,
        metrics,
    })
}

fn bench_case<F>(samples: usize, bytes: usize, mut run: F) -> Result<Metric>
where
    F: FnMut() -> Result<u64>,
{
    let mut timings = Vec::with_capacity(samples);
    let mut checksum = 0_u64;

    let _ = black_box(run()?);
    for _ in 0..samples {
        let start = Instant::now();
        checksum ^= black_box(run()?);
        timings.push(start.elapsed().as_nanos() as u64);
    }

    timings.sort_unstable();
    let median_nanos = timings[timings.len() / 2];
    let min_nanos = timings[0];
    let max_nanos = timings[timings.len() - 1];

    Ok(Metric {
        median_nanos,
        ns_per_byte: median_nanos as f64 / bytes as f64,
        min_nanos,
        max_nanos,
        checksum,
    })
}

#[derive(Clone, Copy)]
enum DecompressMode {
    InitializedLargeOutput,
    UninitLargeOutput,
    VecSpareCapacity,
}

fn chunked_decompress(fixture: &Fixture, mode: DecompressMode) -> Result<u64> {
    let mut decoder = Decompress::new(true);
    let mut result = Vec::with_capacity(fixture.plain.len());

    match mode {
        DecompressMode::InitializedLargeOutput => {
            let mut chunk = vec![0_u8; LARGE_CHUNK_OUT].into_boxed_slice();
            loop {
                let prior_out = decoder.total_out();
                let in_start = decoder.total_in() as usize;
                let in_end = (in_start + CHUNK_IN).min(fixture.zlib.len());
                let status = decoder.decompress(
                    &fixture.zlib[in_start..in_end],
                    &mut chunk,
                    FlushDecompress::None,
                )?;
                let bytes_written = (decoder.total_out() - prior_out) as usize;
                result.extend_from_slice(&chunk[..bytes_written]);
                if status == Status::StreamEnd {
                    break;
                }
            }
        }
        DecompressMode::UninitLargeOutput => {
            let mut chunk = vec![MaybeUninit::<u8>::uninit(); LARGE_CHUNK_OUT].into_boxed_slice();
            loop {
                let prior_out = decoder.total_out();
                let in_start = decoder.total_in() as usize;
                let in_end = (in_start + CHUNK_IN).min(fixture.zlib.len());
                let status = decoder.decompress_uninit(
                    &fixture.zlib[in_start..in_end],
                    &mut chunk,
                    FlushDecompress::None,
                )?;
                let bytes_written = (decoder.total_out() - prior_out) as usize;
                result.extend_from_slice(unsafe {
                    std::slice::from_raw_parts(chunk.as_ptr() as *const u8, bytes_written)
                });
                if status == Status::StreamEnd {
                    break;
                }
            }
        }
        DecompressMode::VecSpareCapacity => loop {
            result.reserve(VEC_RESERVE);
            let in_start = decoder.total_in() as usize;
            let in_end = (in_start + CHUNK_IN).min(fixture.zlib.len());
            let status = decoder.decompress_vec(
                &fixture.zlib[in_start..in_end],
                &mut result,
                FlushDecompress::None,
            )?;
            if status == Status::StreamEnd {
                break;
            }
        },
    }

    ensure!(result == fixture.plain, "decompression output mismatch");
    Ok(hash_bytes(&result) ^ fixture.zlib_hash)
}

#[derive(Clone, Copy)]
enum CompressMode {
    InitializedLargeOutput,
    UninitLargeOutput,
    VecSpareCapacity,
}

fn chunked_compress(fixture: &Fixture, mode: CompressMode) -> Result<u64> {
    let mut encoder = Compress::new(Compression::default(), true);
    let mut result = Vec::with_capacity(fixture.zlib.len() + fixture.plain.len() / 8);

    match mode {
        CompressMode::InitializedLargeOutput => {
            let mut chunk = vec![0_u8; LARGE_CHUNK_OUT].into_boxed_slice();
            loop {
                let prior_out = encoder.total_out();
                let in_start = encoder.total_in() as usize;
                let (input, flush) = compress_input(&fixture.plain, in_start);
                let status = encoder.compress(input, &mut chunk, flush)?;
                let bytes_written = (encoder.total_out() - prior_out) as usize;
                result.extend_from_slice(&chunk[..bytes_written]);
                if status == Status::StreamEnd {
                    break;
                }
            }
        }
        CompressMode::UninitLargeOutput => {
            let mut chunk = vec![MaybeUninit::<u8>::uninit(); LARGE_CHUNK_OUT].into_boxed_slice();
            loop {
                let prior_out = encoder.total_out();
                let in_start = encoder.total_in() as usize;
                let (input, flush) = compress_input(&fixture.plain, in_start);
                let status = encoder.compress_uninit(input, &mut chunk, flush)?;
                let bytes_written = (encoder.total_out() - prior_out) as usize;
                result.extend_from_slice(unsafe {
                    std::slice::from_raw_parts(chunk.as_ptr() as *const u8, bytes_written)
                });
                if status == Status::StreamEnd {
                    break;
                }
            }
        }
        CompressMode::VecSpareCapacity => loop {
            result.reserve(VEC_RESERVE);
            let in_start = encoder.total_in() as usize;
            let (input, flush) = compress_input(&fixture.plain, in_start);
            let status = encoder.compress_vec(input, &mut result, flush)?;
            if status == Status::StreamEnd {
                break;
            }
        },
    }

    let roundtripped = decompress_fixture(&result)?;
    ensure!(roundtripped == fixture.plain, "compression roundtrip mismatch");
    Ok(hash_bytes(&result) ^ fixture.plain_hash)
}

fn compress_input(plain: &[u8], in_start: usize) -> (&[u8], FlushCompress) {
    if in_start >= plain.len() {
        (&[], FlushCompress::Finish)
    } else {
        let in_end = (in_start + CHUNK_IN).min(plain.len());
        (&plain[in_start..in_end], FlushCompress::None)
    }
}

fn compress_fixture(plain: &[u8]) -> Result<Vec<u8>> {
    let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
    encoder
        .write_all(plain)
        .context("failed to create zlib fixture")?;
    encoder.finish().context("failed to finalize zlib fixture")
}

fn decompress_fixture(input: &[u8]) -> Result<Vec<u8>> {
    let mut decoder = ZlibDecoder::new(std::io::Cursor::new(input));
    let mut output = Vec::new();
    decoder
        .read_to_end(&mut output)
        .context("failed to decompress zlib fixture")?;
    Ok(output)
}

fn validate_against_baseline(report: &BenchmarkReport, path: &PathBuf) -> Result<()> {
    let baseline = serde_json::from_reader::<_, serde_json::Value>(
        std::fs::File::open(path).with_context(|| format!("failed to open {}", path.display()))?,
    )
    .with_context(|| format!("failed to parse {}", path.display()))?;

    let backend = report.backend;
    let suite = report.suite;
    let suite_baselines = baseline
        .get("suite_baselines")
        .and_then(|value| value.get(suite))
        .and_then(|value| value.get(backend))
        .context("missing suite baseline entry")?;

    for (name, metric) in &report.metrics {
        let expected = suite_baselines
            .get(name)
            .and_then(|value| value.get("ns_per_byte"))
            .and_then(serde_json::Value::as_f64)
            .with_context(|| format!("missing ns_per_byte baseline for {suite}/{backend}/{name}"))?;
        let ratio = metric.ns_per_byte / expected;
        ensure!(
            (0.6..=1.75).contains(&ratio),
            "{suite}/{backend}/{name} drifted too far from baseline: {ratio:.2}x"
        );
    }

    if report.suite == "current" && report.backend == "zlib-rs" {
        let historical = baseline
            .get("historical_baseline")
            .and_then(|value| value.get("zlib-rs"))
            .and_then(|value| value.get("chunked_decompress_with_large_output_buf"))
            .and_then(|value| value.get("ns_per_byte"))
            .and_then(serde_json::Value::as_f64)
            .context("missing historical zlib-rs baseline")?;
        let issue_metric = report
            .metrics
            .get("chunked_decompress_uninit_with_large_output_buf")
            .context("missing current zlib-rs issue metric")?;
        let ratio = issue_metric.ns_per_byte / historical;
        ensure!(
            ratio >= 2.0,
            "expected issue #544 to reproduce against historical zlib-rs baseline, got {ratio:.2}x"
        );
    }

    Ok(())
}

fn hash_bytes(bytes: &[u8]) -> u64 {
    let mut hasher = DefaultHasher::new();
    bytes.hash(&mut hasher);
    hasher.finish()
}

fn make_plain_data() -> Vec<u8> {
    let mut state = 0x1234_5678_9abc_def0_u64;
    let mut plain = Vec::with_capacity(PLAIN_LEN);
    while plain.len() < PLAIN_LEN {
        state ^= state << 7;
        state ^= state >> 9;
        state ^= state << 8;
        plain.extend_from_slice(&state.to_le_bytes());
    }
    plain.truncate(PLAIN_LEN);
    plain
}

fn backend_name() -> &'static str {
    if cfg!(feature = "flate2-cloudflare-zlib") {
        "cloudflare-zlib"
    } else if cfg!(feature = "flate2-zlib-ng") {
        "zlib-ng"
    } else if cfg!(feature = "flate2-zlib-rs") {
        "zlib-rs"
    } else if cfg!(feature = "flate2-zlib") {
        "zlib"
    } else {
        "rust_backend"
    }
}
