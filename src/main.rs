//#![allow(unused)]
use clap::{Parser, Subcommand};

use self::concat::concat;

mod concat;
mod i2s;
//mod pps;
mod umc;
mod cut_one;

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
    CutOne(CutOneArgs)
}

#[derive(clap::Args)]
struct CutOneArgs {
    /// Path to output file
    #[arg(short, long)]
    output: String,
    /// Path to input file
    #[arg(short, long)]
    input_dir: String,
    /// Start sample
    #[arg(short, long)]
    start: u64,
    /// Number of samples to write
    #[arg(short, long)]
    samples: u64
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
    /// Start time as nanos from epoch
    #[arg(short, long)]
    start: Option<i64>,
    /// Number of samples to write
    #[arg(short, long)]
    samples: Option<u64>,
    /// 'umc' or 'i2s'
    #[arg(short, long)]
    mode: String,
}

#[derive(Debug, serde::Deserialize)]
struct Record {
    time: i64,
    sample: u64,
    file_sample: u32,
    file: String,
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Cut(args) => {
            let mode = args.mode;
            if mode == "umc" {
                umc::make_wav(
                    args.output,
                    args.input_dir,
                    args.clock_file,
                    args.start,
                    args.samples,
                );
            } else if mode == "i2s" {
                i2s::make_wav(
                    args.output,
                    args.input_dir,
                    args.clock_file,
                    args.start,
                    args.samples,
                );
            } else {
                eprintln!("Mode can be 'umc', 'i2s' or 'concat'");
            }
        }
        Commands::Concat(args) => {
            concat(args.input_dir, args.output, args.clock_file, args.step.unwrap_or(1));
        }
        Commands::CutOne(args) => {

        }
    }
}
