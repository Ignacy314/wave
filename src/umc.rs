use std::fs::{self, File};
use std::io::BufWriter;
use std::path::{Path, PathBuf};

use chrono::{DateTime, FixedOffset};
use hound::WavSpec;
use indicatif::{ProgressBar, ProgressStyle};

use crate::pps::{find_best, find_start, Pps};

const AUDIO_PER_DRONE_SAMPLES: u32 = 2400;

struct Cursor {
    audio_sample: u32,
    drone_sample: u32,
    breaks: Vec<(i64, i64)>,
    index: usize,
    spec: WavSpec,
    writer: hound::WavWriter<BufWriter<File>>,
    current_start: u32,
    filename: String,
}

fn wav_file_to_nanos(f: &Path) -> i64 {
    let str = f.file_name().unwrap().to_str().unwrap();
    let str = &str[..str.len() - 4];
    str.parse::<i64>().unwrap()
}

struct ProcessResult {
    write_samples_from_curr: u32,
    advance_files: u32,
    advance_samples_by: u32,
    pos_in_end_file: u32,
}

impl Cursor {
    fn new(filename: String, spec: hound::WavSpec) -> Self {
        let writer = hound::WavWriter::create(filename.clone(), spec).unwrap();
        Self {
            audio_sample: 1,
            drone_sample: 1,
            breaks: vec![],
            index: 0,
            spec,
            writer,
            current_start: 1,
            filename,
        }
    }

    fn add_break(&mut self, start: i64, end: i64) {
        self.breaks.push((start, end));
    }

    fn advance_by(&mut self, samples: u32) {
        self.audio_sample += samples;
        while self.audio_sample > AUDIO_PER_DRONE_SAMPLES {
            self.drone_sample += 1;
            self.audio_sample -= AUDIO_PER_DRONE_SAMPLES;
        }
    }

    fn write_sample(&mut self, sample: i32) {
        self.writer.write_sample(sample).unwrap();
        self.advance_by(1);
    }

    fn finalize_writer(mut self, advance_by: u32) -> Option<Self> {
        self.writer.finalize().unwrap();
        let new_filename =
            format!("{}_{}-{}.wav", self.filename, self.current_start, self.drone_sample);
        fs::rename(self.filename.clone(), new_filename).unwrap();
        if advance_by == u32::MAX {
            return None;
        }
        self.writer = hound::WavWriter::create(self.filename.clone(), self.spec).unwrap();
        self.advance_by(advance_by);
        self.current_start = self.drone_sample;
        Some(self)
    }

    fn process_error(&mut self, curr: usize, waves: &[PathBuf]) -> Option<ProcessResult> {
        if self.index >= self.breaks.len() {
            return None;
        }
        let (start, end) = self.breaks[self.index];
        if waves.len() - 1 == curr {
            return None;
        }
        #[allow(clippy::cast_possible_truncation)]
        #[allow(clippy::cast_sign_loss)]
        if wav_file_to_nanos(&waves[curr + 1]) > start {
            self.index += 1;
            let curr_nanos = wav_file_to_nanos(&waves[curr]);
            let write_nanos_from_curr = start - curr_nanos;
            let write_samples_from_curr = (write_nanos_from_curr * 48 / 1_000_000) as u32;

            let advance_samples_by = ((end - start) * 48 / 1_000_000) as u32;

            let mut i = 0;
            let mut end_file_nanos = curr_nanos;
            let mut next_file_nanos = wav_file_to_nanos(&waves[curr + 1]);
            while next_file_nanos <= end {
                i += 1;
                if curr + i + 1 == waves.len() {
                    return None;
                }
                end_file_nanos = next_file_nanos;
                next_file_nanos = wav_file_to_nanos(&waves[curr + i + 1]);
            }
            let nanos_pos_in_end_file = end - end_file_nanos;
            let pos_in_end_file = (nanos_pos_in_end_file * 48 / 1_000_000) as u32;
            println!("{} {} {} {} {} {}", start, end, write_nanos_from_curr, end_file_nanos, pos_in_end_file, self.drone_sample);
            return Some(ProcessResult {
                write_samples_from_curr,
                advance_files: i as u32,
                advance_samples_by,
                pos_in_end_file,
            });
        }
        None
    }
}

#[allow(unused)]
#[derive(Debug, serde::Deserialize)]
struct Record {
    clock: f64,
    lon: f64,
    lat: f64,
    alt: f64,
    rfc: String,
}

#[derive(Debug, serde::Deserialize)]
struct ErrorTime {
    start: i64,
    end: i64,
}

#[allow(clippy::too_many_lines)]
pub fn make_wav<P: std::convert::AsRef<Path>>(
    timestamps: Option<(DateTime<FixedOffset>, DateTime<FixedOffset>)>,
    output: P,
    input_dir: P,
    csv: Option<P>,
    errors: Option<P>,
) {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: 48000,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Int,
    };

    let mut cursor = Cursor::new(output.as_ref().to_str().unwrap().to_owned(), spec);

    let (from_nanos, to_nanos) = if let Some(timestamps) = timestamps {
        (
            timestamps.0.timestamp_nanos_opt().unwrap(),
            timestamps.1.timestamp_nanos_opt().unwrap(),
        )
    } else if let Some(csv_path) = csv {
        let mut rdr = csv::Reader::from_path(csv_path).unwrap();
        let mut counter = 0;
        let mut time_counter = 0i64;
        let mut rfc = String::new();
        #[allow(clippy::explicit_counter_loop)]
        for result in rdr.deserialize() {
            counter += 1;
            let record: Record = result.unwrap();
            if rfc.is_empty() {
                rfc = record.rfc;
                time_counter += 1;
            } else if record.rfc == rfc {
                time_counter += 1;
            }
        }
        let dt = DateTime::parse_from_rfc3339(&rfc).unwrap();
        time_counter = 20 - time_counter;
        let start = dt.timestamp_nanos_opt().unwrap() + 50_000_000 * time_counter;
        let end = start + (counter - 1) * 50_000_000;
        println!("{start}");
        (start, end)
    } else {
        (0, 0)
    };

    if let Some(errors) = errors {
        let mut rdr = csv::Reader::from_path(errors).unwrap();
        for res in rdr.deserialize() {
            let record: ErrorTime = res.unwrap();
            cursor.add_break(record.start, record.end);
        }
    }

    let (best_pps, mut _best_diff, waves) = find_best(input_dir.as_ref(), from_nanos);

    eprintln!("{best_pps:?}");

    let channels = 2;

    if let Some(Pps { nanos, sample, file }) = best_pps {
        let (start_file, start_sample) =
            find_start(from_nanos, nanos, sample, &file, &waves, channels, 48000.0);

        #[allow(clippy::cast_precision_loss)]
        #[allow(clippy::cast_possible_truncation)]
        #[allow(clippy::cast_sign_loss)]
        let mut samples_left = ((to_nanos - from_nanos) as f64 / 1e9_f64 * 48000.0).round() as u32;

        let pb = ProgressBar::new(u64::from(samples_left));
        #[allow(clippy::cast_possible_truncation)]
        #[allow(clippy::cast_sign_loss)]
        let t = f64::from(samples_left).log10().ceil() as u64;
        pb.set_style(
            ProgressStyle::with_template(&format!(
            "[{{elapsed_precise}}] {{bar:40.cyan/blue}} {{pos:>{t}}}/{{len:{t}}} ({{percent}}%) {{msg}}"
        ))
            .unwrap()
            .progress_chars("##-"),
        );

        let mut start = true;
        let mut skip = 0;
        let mut end = false;
        let mut pps;
        let mut pos_in_file;
        let mut process_res: Option<ProcessResult> = None;
        for (i, wav) in waves.iter().skip_while(|x| **x != start_file).enumerate() {
            if let Some(res) = process_res.as_mut() {
                if res.advance_files > 0 {
                    res.advance_files -= 1;
                    continue;
                }
            } else {
                process_res = cursor.process_error(i, &waves);
            }
            pos_in_file = 1u32;
            pps = false;
            let mut reader = match hound::WavReader::open(wav) {
                Ok(r) => r,
                Err(_e) => {
                    continue;
                }
            };
            if start {
                reader.seek(start_sample / channels).unwrap();
                start = false;
            }
            for s in reader.samples::<i32>() {
                let sample = s.unwrap();
                #[allow(clippy::cast_possible_wrap)]
                if sample == 0xeeee_eeee_u32 as i32 {
                    if pps {
                        pps = false;
                        skip += 2;
                    } else {
                        pps = true;
                    }
                } else if skip > 0 {
                    skip -= 1;
                } else {
                    if let Some(res) = process_res.as_mut() {
                        if res.write_samples_from_curr == 0 {
                            if res.advance_files != 0 {
                                break;
                            }
                            if pos_in_file < res.pos_in_end_file {
                                pos_in_file += 1;
                                continue;
                            }
                            cursor = cursor.finalize_writer(res.advance_samples_by).unwrap();
                            samples_left -= res.advance_samples_by;
                            process_res = None;
                        } else {
                            res.write_samples_from_curr -= 1;
                        }
                    }
                    pos_in_file += 1;
                    skip += 1;
                    cursor.write_sample(sample);
                    samples_left -= 1;
                    pb.inc(1);
                    if samples_left == 0 {
                        end = true;
                        break;
                    }
                }
            }
            if end {
                break;
            }
        }

        cursor.finalize_writer(u32::MAX);
        pb.finish_with_message(format!("Samples left: {samples_left}"));
    }
}
