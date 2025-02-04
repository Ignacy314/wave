//#![allow(unused)]
use clap::Parser;

use chrono::DateTime;

mod i2s;
mod pps;
mod umc;

#[derive(Parser, Debug)]
struct Args {
    /// Start timestamp in format "%Y-%m-%d %H:%M:%S%.3f %z"
    start_time: String,
    /// End timestamp in the same format as start timestamp
    end_time: String,
    /// Path to output file (without .wav extension - for i2s mode it's the base name for all output files)
    output: String,
    /// Path to input directory containing wav files with names being numbers of nanoseconds since unix epoch
    input_dir: String,
    /// 'umc' or 'i2s'
    mode: String,
    #[arg(short, long)]
    /// Path to directory containing csv files describing drone flights
    csv_dir: Option<String>,
    #[arg(short, long)]
    /// Path to file containing in each row a single timestamp (nanoseconds since unix epoch) of an
    /// alsa error
    errors_file: Option<String>
}

fn main() {
    let args = Args::parse();
    let from = DateTime::parse_from_str(&args.start_time, "%Y-%m-%d %H:%M:%S%.3f %z").unwrap();
    let to = DateTime::parse_from_str(&args.end_time, "%Y-%m-%d %H:%M:%S%.3f %z").unwrap();
    let output = args.output;
    let input_dir = args.input_dir;

    let mode = args.mode;
    if mode == "umc" {
        umc::make_wav(from, to, output, input_dir);
    } else if mode == "i2s" {
        i2s::make_wav(from, to, output, input_dir);
    } else {
        eprintln!("Specify either 'umc' or 'i2s' after input dir");
    }
}
