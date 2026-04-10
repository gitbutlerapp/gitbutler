use std::{
    env,
    fs::File,
    io::{self, BufReader},
    path::{Path, PathBuf},
    time::Instant,
};

use flate2::{
    bufread::GzDecoder as BufReadGzDecoder, read::GzDecoder as ReadGzDecoder, Decompress,
    FlushDecompress, Status,
};
use tar::Archive;

#[cfg(all(feature = "zlib-ng-backend", feature = "zlib-rs-backend"))]
compile_error!("enable exactly one flate2 backend feature");

#[cfg(not(any(feature = "zlib-ng-backend", feature = "zlib-rs-backend")))]
compile_error!("enable either the zlib-ng-backend or zlib-rs-backend feature");

const WARMUP_ITERATIONS: usize = 1;

type BenchFn = fn(&Path) -> io::Result<u64>;

struct Options {
    input: PathBuf,
    iterations: usize,
    csv: Option<PathBuf>,
}

struct Case {
    name: &'static str,
    bench: BenchFn,
}

struct Row {
    case: &'static str,
    iteration: usize,
    elapsed_ns: u128,
    output_bytes: u64,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let options = parse_args(env::args().skip(1))?;
    let rows = run_benchmarks(&options)?;
    write_csv(&rows, options.csv.as_deref())?;
    Ok(())
}

fn parse_args<I>(mut args: I) -> Result<Options, Box<dyn std::error::Error>>
where
    I: Iterator<Item = String>,
{
    let mut input = None;
    let mut iterations = 6usize;
    let mut csv = None;

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--input" => input = Some(PathBuf::from(next_arg("--input", &mut args)?)),
            "--iterations" => iterations = next_arg("--iterations", &mut args)?.parse()?,
            "--csv" => csv = Some(PathBuf::from(next_arg("--csv", &mut args)?)),
            "--help" | "-h" => {
                print_help();
                std::process::exit(0);
            }
            other => return Err(format!("unknown argument: {other}").into()),
        }
    }

    let input = input.ok_or("missing required --input argument")?;
    if iterations == 0 {
        return Err("iterations must be greater than zero".into());
    }

    Ok(Options {
        input,
        iterations,
        csv
    })
}

fn next_arg<I>(flag: &str, args: &mut I) -> Result<String, Box<dyn std::error::Error>>
where
    I: Iterator<Item = String>,
{
    args.next()
        .ok_or_else(|| format!("missing value for {flag}").into())
}

fn print_help() {
    eprintln!(
        "usage: cargo run --release -- --input <path> [--iterations <count>] [--csv <path>]"
    );
}

fn run_benchmarks(options: &Options) -> io::Result<Vec<Row>> {
    let cases = [
        Case {
            name: "gzip_read_copy",
            bench: bench_read_decoder_copy
        },
        Case {
            name: "gzip_read_small_chunks",
            bench: bench_read_decoder_small_chunks
        },
        Case {
            name: "gzip_decompress_small_output",
            bench: bench_decompress_small_output
        },
        Case {
            name: "gzip_bufread_copy",
            bench: bench_bufread_decoder_copy
        },
        Case {
            name: "tar_archive_entries",
            bench: bench_tar_archive_entries
        },
        Case {
            name: "tar_archive_entries_bufread",
            bench: bench_tar_archive_entries_bufread
        },
    ];
    let mut rows = Vec::with_capacity(cases.len() * options.iterations);

    for case in cases {
        for _ in 0..WARMUP_ITERATIONS {
            (case.bench)(&options.input)?;
        }

        for iteration in 0..options.iterations {
            let start = Instant::now();
            let output_bytes = (case.bench)(&options.input)?;
            rows.push(Row {
                case: case.name,
                iteration,
                elapsed_ns: start.elapsed().as_nanos(),
                output_bytes,
            });
        }
    }

    Ok(rows)
}

fn bench_read_decoder_copy(input: &Path) -> io::Result<u64> {
    let file = File::open(input)?;
    let mut decoder = ReadGzDecoder::new(file);
    io::copy(&mut decoder, &mut io::sink())
}

fn bench_read_decoder_small_chunks(input: &Path) -> io::Result<u64> {
    let file = File::open(input)?;
    let mut decoder = ReadGzDecoder::new(file);
    let mut output_bytes = 0;
    let mut buffer = [0u8; 64];
    loop {
        let read = io::Read::read(&mut decoder, &mut buffer)?;
        if read == 0 {
            break;
        }
        output_bytes += read as u64;
    }
    Ok(output_bytes)
}

fn bench_decompress_small_output(input: &Path) -> io::Result<u64> {
    let input = std::fs::read(input)?;
    let mut decompressor = Decompress::new_gzip(15);
    let mut input_offset = 0usize;
    let mut output_bytes = 0u64;
    let mut buffer = [0u8; 64];

    loop {
        let before_in = decompressor.total_in();
        let before_out = decompressor.total_out();
        let flush = if input_offset == input.len() {
            FlushDecompress::Finish
        } else {
            FlushDecompress::None
        };
        let status = decompressor
            .decompress(&input[input_offset..], &mut buffer, flush)
            .map_err(io::Error::other)?;

        input_offset += (decompressor.total_in() - before_in) as usize;
        output_bytes += decompressor.total_out() - before_out;

        if status == Status::StreamEnd {
            break;
        }

        if input_offset == input.len()
            && status == Status::BufError
            && decompressor.total_out() == before_out
        {
            return Err(io::Error::other(
                "decompressor made no progress while processing the entire input",
            ));
        }
    }

    Ok(output_bytes)
}

fn bench_bufread_decoder_copy(input: &Path) -> io::Result<u64> {
    let file = File::open(input)?;
    let reader = BufReader::new(file);
    let mut decoder = BufReadGzDecoder::new(reader);
    io::copy(&mut decoder, &mut io::sink())
}

fn bench_tar_archive_entries(input: &Path) -> io::Result<u64> {
    let file = File::open(input)?;
    let decoder = ReadGzDecoder::new(file);
    let mut archive = Archive::new(decoder);
    let mut output_bytes = 0;
    for entry in archive.entries()? {
        let mut entry = entry?;
        output_bytes += io::copy(&mut entry, &mut io::sink())?;
    }
    Ok(output_bytes)
}

fn bench_tar_archive_entries_bufread(input: &Path) -> io::Result<u64> {
    let file = File::open(input)?;
    let reader = BufReader::new(file);
    let decoder = BufReadGzDecoder::new(reader);
    let mut archive = Archive::new(decoder);
    let mut output_bytes = 0;
    for entry in archive.entries()? {
        let mut entry = entry?;
        output_bytes += io::copy(&mut entry, &mut io::sink())?;
    }
    Ok(output_bytes)
}

fn write_csv(rows: &[Row], csv_path: Option<&Path>) -> io::Result<()> {
    let mut csv = String::from("backend,case,iteration,elapsed_ns,output_bytes\n");
    for row in rows {
        csv.push_str(backend_name());
        csv.push(',');
        csv.push_str(row.case);
        csv.push(',');
        csv.push_str(&row.iteration.to_string());
        csv.push(',');
        csv.push_str(&row.elapsed_ns.to_string());
        csv.push(',');
        csv.push_str(&row.output_bytes.to_string());
        csv.push('\n');
    }

    if let Some(path) = csv_path {
        std::fs::write(path, csv)
    } else {
        print!("{csv}");
        Ok(())
    }
}

fn backend_name() -> &'static str {
    #[cfg(feature = "zlib-ng-backend")]
    {
        "zlib-ng"
    }
    #[cfg(feature = "zlib-rs-backend")]
    {
        "zlib-rs"
    }
}
