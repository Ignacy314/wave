//#![allow(unused)]
use clap::{Parser, Subcommand};

use chrono::DateTime;

use self::concat::concat;

mod i2s;
mod pps;
mod umc;
mod concat;

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
    Cut(Args),
}

#[derive(clap::Args)]
struct ConcatArgs {
    #[arg(short, long)]
    /// Path to output file
    output: String,
    #[arg(short, long)]
    /// Path to input directory containing wav files
    input_dir: String,
}

#[derive(clap::Args)]
struct Args {
    #[clap(flatten)]
    time_or_csv: TimeOrCsv,
    #[arg(short, long)]
    /// Path to output file (without .wav extension - for i2s mode it's the base name for all output files)
    output: String,
    #[arg(short, long)]
    /// Path to input directory containing wav files with names being numbers of nanoseconds since unix epoch
    input_dir: String,
    #[arg(short, long)]
    /// 'umc' or 'i2s'
    mode: String,
    #[arg(short, long)]
    /// Path to file containing in each row a single timestamp (nanoseconds since unix epoch) of an
    /// alsa error
    errors_file: Option<String>,
}

#[derive(Debug, clap::Args)]
#[group(required = true, multiple = false)]
struct TimeOrCsv {
    #[arg(
        short,
        long,
        value_delimiter = ' ',
        num_args = 2,
        value_name = "TIMESTAMP"
    )]
    /// Timestamps of start and end in the format "%Y-%m-%d %H:%M:%S%.3f %z" delimited by a
    /// whitespace
    timestamps: Option<Vec<String>>,
    #[arg(short, long)]
    /// Path to directory containing csv files describing drone flights
    csv_file: Option<String>,
}

fn main() {
    //let args = Args::parse();
    let cli = Cli::parse();

    match cli.command {
        Commands::Cut(args) => {
            let timestamps = if let Some(timestamps) = args.time_or_csv.timestamps {
                //let from = DateTime::parse_from_str(&timestamps[0], "%Y-%m-%d %H:%M:%S%.3f %z").unwrap();
                //let to = DateTime::parse_from_str(&timestamps[1], "%Y-%m-%d %H:%M:%S%.3f %z").unwrap();
                let from = DateTime::parse_from_rfc3339(&timestamps[0]).unwrap();
                let to = DateTime::parse_from_rfc3339(&timestamps[1]).unwrap();
                Some((from, to))
            } else {
                None
            };

            let output = args.output;
            let input_dir = args.input_dir;

            let mode = args.mode;
            if mode == "umc" {
                umc::make_wav(
                    timestamps,
                    output,
                    input_dir,
                    args.time_or_csv.csv_file,
                    args.errors_file,
                );
            } else if mode == "i2s" {
                i2s::make_wav(timestamps, output, input_dir);
            } else if mode == "concat" {
            } else {
                eprintln!("Mode can be 'umc', 'i2s' or 'concat'");
            }
        }
        Commands::Concat(args) => {
            concat(args.input_dir, args.output);
        },
    }
}
