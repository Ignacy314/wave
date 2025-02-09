use std::fs::{self, File};
use std::io::BufWriter;
use std::path::Path;

use chrono::{DateTime, FixedOffset};
use hound::WavSpec;
use indicatif::{ProgressBar, ProgressStyle};

use crate::pps::{find_best, find_start, Pps};

const AUDIO_PER_DRONE_SAMPLES: u32 = 2400;

#[derive(Clone, Copy, Debug)]
struct Break {
    start: (i64, u32),
    end: (i64, u32),
    len: i64,
}

struct Cursor {
    audio_sample: u32,
    drone_sample: u32,
    breaks: Vec<Break>,
    index: usize,
    spec: WavSpec,
    writer: Option<hound::WavWriter<BufWriter<File>>>,
    current_start: u32,
    filename: String,
    current_break: Option<Break>,
}

fn wav_file_to_nanos(f: &Path) -> i64 {
    let str = f.file_name().unwrap().to_str().unwrap();
    let str = &str[..str.len() - 4];
    str.parse::<i64>().unwrap()
}

struct ProcessResult {
    write_samples_from_curr: u32,
    is_end_file: bool,
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
            writer: Some(writer),
            current_start: 1,
            filename,
            current_break: None,
        }
    }

    fn add_break(&mut self, b_start: i64, b_end: i64, dir: &Path) {
        let (start_pps, _, start_waves) = find_best(dir, b_start);
        if let Some(Pps { nanos, sample, file }) = start_pps {
            let (start_file, start_sample) =
                find_start(b_start, nanos, sample, &file, &start_waves, 2, 48000.0);

            let (end_pps, _, end_waves) = find_best(dir, b_end);
            if let Some(Pps { nanos, sample, file }) = end_pps {
                let (end_file, end_sample) =
                    find_start(b_end, nanos, sample, &file, &end_waves, 2, 48000.0);
                let br = Break {
                    start: (wav_file_to_nanos(&start_file), start_sample),
                    end: (wav_file_to_nanos(&end_file), end_sample),
                    len: b_end - b_start,
                };
                println!("{br:?}");
                self.breaks.push(Break {
                    start: (wav_file_to_nanos(&start_file), start_sample),
                    end: (wav_file_to_nanos(&end_file), end_sample),
                    len: b_end - b_start,
                });
            } else {
                panic!("Failed to find end point of a break");
            }
        } else {
            panic!("Failed to find start point of a break");
        }
    }

    fn advance_by(&mut self, samples: u32) {
        self.audio_sample += samples;
        while self.audio_sample > AUDIO_PER_DRONE_SAMPLES {
            self.drone_sample += 1;
            self.audio_sample -= AUDIO_PER_DRONE_SAMPLES;
        }
    }

    fn write_sample(&mut self, sample: i32) {
        if let Some(writer) = self.writer.as_mut() {
            writer.write_sample(sample).unwrap();
        }
        self.advance_by(1);
    }

    fn finalize_writer(&mut self, advance_by: u32) {
        let writer = self.writer.take();
        writer.unwrap().finalize().unwrap();
        let new_filename =
            format!("{}_{}-{}.wav", self.filename, self.current_start, self.drone_sample);
        fs::rename(self.filename.clone(), new_filename).unwrap();
        if advance_by == u32::MAX {
            return;
        }
        self.writer = Some(hound::WavWriter::create(self.filename.clone(), self.spec).unwrap());
        self.advance_by(advance_by);
        self.current_start = self.drone_sample;
    }

    fn process_error(&mut self, curr_file: &Path) -> Option<ProcessResult> {
        let curr_nanos = wav_file_to_nanos(curr_file);
        if let Some(br) = self.current_break {
            if curr_nanos == br.end.0 {
                return Some(ProcessResult {
                    write_samples_from_curr: 0,
                    is_end_file: true,
                    pos_in_end_file: br.end.1,
                });
            }
        }
        if self.index >= self.breaks.len() {
            return None;
        }
        let br = self.breaks[self.index];

        if curr_nanos == br.start.0 {
            self.index += 1;
            self.current_break = Some(br);
            //println!("{curr_nanos}: {br:?}");
            return Some(ProcessResult {
                write_samples_from_curr: br.start.1,
                is_end_file: br.end.0 == curr_nanos,
                pos_in_end_file: br.end.1,
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
        (start, end)
    } else {
        (0, 0)
    };

    if let Some(errors) = errors {
        let mut rdr = csv::Reader::from_path(errors).unwrap();
        for res in rdr.deserialize() {
            let record: ErrorTime = res.unwrap();
            cursor.add_break(record.start, record.end, input_dir.as_ref());
        }
    }

    let (best_pps, mut _best_diff, waves) = find_best(input_dir.as_ref(), from_nanos);

    eprintln!("{best_pps:?}");

    let channels = 2;

    if let Some(Pps { nanos, sample, file }) = best_pps {
        let (start_file, start_sample) =
            find_start(from_nanos, nanos, sample, &file, &waves, channels, 48000.0);
        //println!("{} {start_sample}", start_file.to_str().unwrap());

        let mut samples_left = ((to_nanos - from_nanos) as f64 / 1e9_f64 * 48000.0).round() as u32;

        let pb = ProgressBar::new(u64::from(samples_left));
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
        for wav in waves.iter().skip_while(|x| **x != start_file) {
            let mut process_res = cursor.process_error(wav);
            if let Some(res) = process_res.as_ref() {
                if res.write_samples_from_curr == 0 && !res.is_end_file {
                    continue;
                }
            }
            let mut pos_in_file = 1u32;
            pps = false;
            let mut reader = match hound::WavReader::open(wav) {
                Ok(r) => r,
                Err(_e) => {
                    continue;
                }
            };
            if start {
                reader.seek(start_sample).unwrap();
                start = false;
            }
            for s in reader.samples::<i32>() {
                let sample = s.unwrap();
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
                        if pos_in_file > res.write_samples_from_curr {
                            if !res.is_end_file {
                                break;
                            } else if pos_in_file > res.pos_in_end_file {
                                //println!("{}: {:?}", wav_file_to_nanos(wav), cursor.current_break);
                                println!("{samples_left}");
                                cursor.finalize_writer(
                                    (cursor.current_break.unwrap().len * 48 / 1_000_000) as u32,
                                );
                                cursor.current_break = None;
                                process_res = None;
                                continue;
                            } else {
                                pos_in_file += 1;
                                continue;
                            }
                        }
                    }
                    cursor.write_sample(sample);
                    skip += 1;
                    pos_in_file += 1;
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
