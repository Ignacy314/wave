use chrono::DateTime;
//#![allow(unused)]
use clap::{Parser, Subcommand};

use self::concat::concat;

mod concat;
mod i2s;
//mod pps;
mod cut_one;
mod umc;

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
#[command(propagate_version = true)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Concatenates all wav files in a folder
    Concat(ConcatArgs),
    /// Concat and optionally cut waves while removing PPS
    Cut(Args),
    /// Cut one file
    CutOne(CutOneArgs),
}

#[derive(clap::Args)]
struct CutOneArgs {
    /// Path to output file
    #[arg(short, long)]
    output: String,
    /// Path to input file
    #[arg(short, long)]
    input: String,
    /// Start sample
    #[arg(short, long)]
    start: u32,
    /// Number of samples to write
    #[arg(short, long)]
    samples: u64,
}

#[derive(clap::Args)]
struct ConcatArgs {
    /// Path to output file
    #[arg(short, long)]
    output: String,
    /// Path to input directory containing wav files
    #[arg(short, long)]
    input_dir: String,
    /// Path to a csv clock file
    #[arg(short, long)]
    clock_file: String,
    /// Step by that many samples
    #[arg(short, long)]
    step: Option<usize>,
}

#[derive(clap::Args)]
struct Args {
    /// Path to output file (without .wav extension - for i2s mode it's the base name for all output files)
    #[arg(short, long)]
    output: String,
    /// Path to input directory containing wav files with names being numbers of nanoseconds since unix epoch
    #[arg(short, long)]
    input_dir: String,
    /// Path to a csv clock file
    #[arg(short, long)]
    clock_file: String,
    /// 'umc' or 'i2s'
    #[arg(short, long)]
    mode: String,
    /// Start time as nanos from epoch
    #[arg(long)]
    start: Option<i64>,
    #[arg(long)]
    samples: Option<u64>,
    #[arg(long)]
    cuts: Option<String>,
    #[arg(long)]
    module: u8,
}

#[derive(Debug, serde::Deserialize)]
struct Record {
    time: i64,
    sample: u64,
    file_sample: u32,
    file: String,
}

#[derive(Debug, serde::Deserialize)]
struct CutRecord {
    start: String,
    end: String,
    range: String,
}

struct Run {
    start: Option<i64>,
    samples: Option<u64>,
    output_dir_ext: String,
}

fn runs(
    start: Option<i64>,
    samples: Option<u64>,
    cuts: Option<String>,
    mode: &str,
    module: u8,
) -> Vec<Run> {
    let Some(cuts) = cuts else {
        return vec![Run {
            start,
            samples,
            output_dir_ext: format!("{mode}/{module}/D{module}_"),
        }];
    };

    let (channels, sample_rate) = if mode == "rawi2s" {
        (4, 192000)
    } else {
        (1, 48000)
    };

    let mut reader = csv::Reader::from_path(cuts).unwrap();
    let records = reader.deserialize();
    let mut runs = Vec::new();

    for r in records {
        let cut: CutRecord = r.unwrap();
        let start_nanos = DateTime::parse_from_rfc3339(&cut.start)
            .unwrap()
            .timestamp_nanos_opt()
            .unwrap();
        let end_nanos = DateTime::parse_from_rfc3339(&cut.end)
            .unwrap()
            .timestamp_nanos_opt()
            .unwrap();
        let samples =
            ((end_nanos - start_nanos) as f64 / 1e9f64 * channels as f64 * sample_rate as f64)
                .round() as u64;
        runs.push(Run {
            start: Some(start_nanos),
            samples: Some(samples),
            output_dir_ext: format!("{mode}/{module}/{}/D{module}_", cut.range),
        });
    }

    runs
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Cut(args) => {
            for (i, run) in runs(args.start, args.samples, args.cuts, &args.mode, args.module)
                .iter()
                .enumerate()
            {
                let output = format!("{}/{}{i}.wav", &args.output, run.output_dir_ext);
                let mode = &args.mode;
                if mode == "umc" {
                    umc::make_wav(
                        &output,
                        &args.input_dir,
                        &args.clock_file,
                        run.start,
                        run.samples,
                        None,
                        None,
                        None,
                    );
                } else if mode == "i2s" {
                    i2s::make_wav(
                        &output,
                        &args.input_dir,
                        &args.clock_file,
                        run.start,
                        run.samples,
                    );
                } else if mode == "rawi2s" {
                    umc::make_wav(
                        &output,
                        &args.input_dir,
                        &args.clock_file,
                        run.start,
                        run.samples,
                        Some(1),
                        Some(4),
                        Some(192000),
                    );
                } else {
                    eprintln!("Mode can be 'umc', 'i2s' or 'rawi2s'");
                }
            }
        }
        Commands::Concat(args) => {
            concat(args.input_dir, args.output, args.clock_file, args.step.unwrap_or(1));
        }
        Commands::CutOne(args) => {
            cut_one::make_wav(args.output, args.input, args.start, args.samples);
        }
    }
}
