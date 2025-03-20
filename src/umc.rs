use std::path::Path;

use indicatif::{ProgressBar, ProgressStyle};

use crate::Record;

const FREQ: f64 = 48000.0;
//const CHANNELS: u32 = 2;

//fn wav_file_to_nanos(f: &Path) -> i64 {
//    let str = f.file_name().unwrap().to_str().unwrap();
//    let str = &str[..str.len() - 4];
//    str.parse::<i64>().unwrap()
//}

#[allow(clippy::too_many_arguments)]
pub fn make_wav<P: std::convert::AsRef<Path>>(
    output: P,
    input_dir: P,
    clock: P,
    start: Option<i64>,
    samples: Option<u64>,
    step: Option<usize>,
    channels: Option<u16>,
    sample_rate: Option<u32>
) {
    let mut waves = std::fs::read_dir(input_dir.as_ref())
        .unwrap()
        .flat_map(|f| f.map(|e| e.path()))
        .collect::<Vec<_>>();
    waves.sort_unstable();

    let spec = hound::WavSpec {
        channels: channels.unwrap_or(1),
        sample_rate: sample_rate.unwrap_or(48000),
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Int,
    };

    let clock_start_nanos_str = clock.as_ref().file_stem().unwrap().to_str().unwrap();

    let mut wav_iter = waves.iter().peekable();
    while let Some(wav) = wav_iter.peek() {
        if wav.file_stem().unwrap().to_str().unwrap() == clock_start_nanos_str {
            break;
        }
        wav_iter.next();
    }

    if wav_iter.peek().is_none() {
        eprintln!("Clock start not found");
        return;
    }

    let mut reader = csv::Reader::from_path(clock).unwrap();

    let records = reader
        .deserialize()
        .map(|r| r.unwrap())
        .collect::<Vec<Record>>();

    if records.is_empty() {
        eprintln!("Failed to read clock csv");
        return;
    }

    let n_records = records.len();

    let mut start_file = records[0].file.clone();
    let mut file_start_sample = 0;
    if let Some(start) = start {
        let mut diff = i64::MAX;
        for r in records.iter() {
            let r_diff = (r.time - start).abs();
            if r_diff < diff {
                diff = r_diff;
                start_file = r.file.clone();
                let r_diff = start - r.time;
                let sample_diff = (r_diff as f64 * FREQ / 1e9).round() as i64;
                file_start_sample = (r.file_sample as i64 + sample_diff).max(0);
            }
        }
    }
    let start_file = input_dir.as_ref().join(start_file);
    let mut file_start_sample = file_start_sample as u32;

    while let Some(wav) = wav_iter.peek() {
        if **wav == start_file {
            break;
        }
        wav_iter.next();
    }

    //let start_nanos = if let Some(start) = start {
    //    start
    //} else {
    //    records[0].time - (records[0].sample as f64 / FREQ * 1e9).round() as i64
    //};
    let end_file = records[n_records - 1].file.clone();
    let end_file = input_dir.as_ref().join(end_file);

    let mut samples = if let Some(samples) = samples {
        samples
    } else {
        records[n_records - 1].sample
    };

    let pb = ProgressBar::new(samples);
    let t = (samples as f64).log10().ceil() as u64;
    pb.set_style(
        ProgressStyle::with_template(&format!(
            "[{{elapsed_precise}}] {{bar:40.cyan/blue}} {{pos:>{t}}}/{{len:{t}}} ({{percent}}%) {{msg}}"
        ))
        .unwrap()
        .progress_chars("##-"),
    );

    let mut writer = hound::WavWriter::create(output, spec).unwrap();

    let mut start = true;
    let mut end = false;
    for wav in wav_iter {
        let mut reader = match hound::WavReader::open(wav) {
            Ok(r) => r,
            Err(e) => {
                eprintln!("Error reading file: {e}");
                return;
            }
        };

        if start {
            if file_start_sample <= reader.duration() {
                reader.seek(file_start_sample).unwrap();
                start = false;
            } else {
                file_start_sample -= reader.duration();
                continue;
            }
        }

        for s in reader.samples::<i32>().step_by(step.unwrap_or(2)) {
            if samples != 0 {
                writer.write_sample(s.unwrap()).unwrap();
                samples -= 1;
                pb.inc(1);
            } else {
                end = true;
                break;
            }
        }

        if end || *wav == end_file {
            break;
        }
    }
    let samples_processed = pb.position();
    pb.finish_with_message(format!("Samples processed: {samples_processed}"));
}
