use std::path::Path;

use clap::Parser;

use chrono::{DateTime, FixedOffset};
use serde::Deserialize;

#[derive(Parser)]
struct Args {
    /// Start timestamp
    #[arg(short, long)]
    start: Option<String>,
    /// Path to output dir plus file name stem
    #[arg(short, long)]
    output: String,
    /// Path to input wav file
    #[arg(short, long)]
    input: String,
    /// Path to csv with split timestamps
    #[arg(short, long)]
    csv: Option<String>,
}

#[derive(Deserialize)]
struct SplitRecord {
    start: String,
    stop: String,
}

#[derive(Debug)]
struct Split {
    start: DateTime<FixedOffset>,
    stop: DateTime<FixedOffset>,
}

fn time_diff_to_samples(start: DateTime<FixedOffset>, stop: DateTime<FixedOffset>) -> u32 {
    assert!(stop >= start);
    let diff = stop - start;
    let nanos = diff.num_nanoseconds().unwrap();
    (nanos as f64 / 1e9_f64 * 48000.0).round() as u32
}

fn main() {
    let args = Args::parse();

    let start = if let Some(start) = args.start {
        DateTime::parse_from_rfc3339(&start).unwrap()
    } else {
        let path = Path::new(&args.input);
        let filename = path.file_stem().unwrap().to_str().unwrap();
        let split_index = filename.match_indices('_').last().unwrap();
        let timestamp = &filename[split_index.0..];
        DateTime::parse_from_rfc3339(timestamp).unwrap()
    };

    let mut reader = hound::WavReader::open(args.input).unwrap();

    let mut csv = args.csv.map(|csv| csv::Reader::from_path(csv).unwrap());
    let records = csv.as_mut().map(|csv| csv.deserialize());

    let mut splits = Vec::new();
    if let Some(records) = records {
        for record in records {
            let split_record: SplitRecord = record.unwrap();
            let split = Split {
                start: DateTime::parse_from_rfc3339(&split_record.start).unwrap(),
                stop: DateTime::parse_from_rfc3339(&split_record.stop).unwrap(),
            };
            splits.push(split);
        }
    }

    let mut splits_iter = splits.iter();
    let mut next_split = splits_iter.next();

    let mut write = if let Some(next_split) = next_split {
        start < next_split.start
    } else {
        true
    };
    let mut write_until_finish = false;
    let mut samples = if write {
        if let Some(next_split) = next_split {
            time_diff_to_samples(start, next_split.start)
        } else {
            write_until_finish = true;
            0
        }
    } else {
        time_diff_to_samples(next_split.unwrap().start, next_split.unwrap().stop)
    };

    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: 48000,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Int,
    };

    let mut i = 0;

    let mut writer =
        Some(hound::WavWriter::create(format!("{}_{i}.wav", args.output), spec).unwrap());

    for sample in reader.samples::<i32>().step_by(2) {
        let s = sample.unwrap();

        if !write_until_finish && samples == 0 {
            if write {
                samples = time_diff_to_samples(next_split.unwrap().start, next_split.unwrap().stop);
                println!("skip: {samples}");
                write = false;
                writer.unwrap().finalize().unwrap();
                writer = None;
            } else {
                let start = next_split.unwrap().stop;
                if let Some(next) = splits_iter.next() {
                    next_split = Some(next);
                    println!("{next_split:?}");
                    samples = time_diff_to_samples(start, next_split.unwrap().start);
                    println!("write: {samples}");
                } else {
                    write_until_finish = true;
                    println!("write to end");
                }
                write = true;
                i += 1;
                writer = Some(
                    hound::WavWriter::create(format!("{}_{i}.wav", args.output), spec).unwrap(),
                );
            }
        }

        if write {
            if let Some(writer) = writer.as_mut() {
                writer.write_sample(s).unwrap();
            }
        }
        samples -= 1;
    }

    //match cli.command {
    //    Commands::Cut(args) => {
    //        let timestamps = if let Some(timestamps) = args.time_or_csv.timestamps {
    //            //let from = DateTime::parse_from_str(&timestamps[0], "%Y-%m-%d %H:%M:%S%.3f %z").unwrap();
    //            //let to = DateTime::parse_from_str(&timestamps[1], "%Y-%m-%d %H:%M:%S%.3f %z").unwrap();
    //            let from = DateTime::parse_from_rfc3339(&timestamps[0]).unwrap();
    //            let to = DateTime::parse_from_rfc3339(&timestamps[1]).unwrap();
    //            Some((from, to))
    //        } else {
    //            None
    //        };
    //
    //        let output = args.output;
    //        let input_dir = args.input_dir;
    //
    //        let mode = args.mode;
    //        if mode == "umc" {
    //            umc::make_wav(
    //                timestamps,
    //                output,
    //                input_dir,
    //                args.time_or_csv.csv_file,
    //                args.errors_file,
    //            );
    //        } else if mode == "i2s" {
    //            i2s::make_wav(timestamps, output, input_dir);
    //        } else if mode == "concat" {
    //        } else {
    //            eprintln!("Mode can be 'umc', 'i2s' or 'concat'");
    //        }
    //    }
    //    Commands::Concat(args) => {
    //        concat(args.input_dir, args.output);
    //    }
    //}
}
