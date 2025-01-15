#![allow(unused)]
use std::fs::{DirEntry, File};
use std::io::BufWriter;
use std::mem::transmute;
use std::path::{Path, PathBuf};
use std::{backtrace, env};

use chrono::{DateTime, FixedOffset, Utc};
use indicatif::{ProgressBar, ProgressStyle};

struct CircularI2S {
    size: usize,
    inner_size: usize,
    buf: Vec<[i32; Self::BUF_SIZE_INNER]>,
    index: usize,
    inner_index: usize,
    filled: bool,
    files: [hound::WavWriter<BufWriter<File>>; Self::BUF_SIZE_INNER + 1],
}

impl CircularI2S {
    const BUF_SIZE: usize = 33;
    const BUF_SIZE_INNER: usize = 8;
    const MID: usize = Self::BUF_SIZE_INNER / 2;

    fn new<P: std::convert::AsRef<PathBuf>>(path: P, num: u8) -> Self {
        let paths: [String; Self::BUF_SIZE_INNER + 1] = (0..=Self::BUF_SIZE_INNER)
            .map(|i| format!("{}_{num}_{i}", path.as_ref().display()))
            .collect::<Vec<_>>()
            .try_into()
            .unwrap();
        let spec = hound::WavSpec {
            channels: 1,
            sample_rate: 48000,
            bits_per_sample: 32,
            sample_format: hound::SampleFormat::Int,
        };
        let files: [hound::WavWriter<BufWriter<File>>; Self::BUF_SIZE_INNER + 1] = paths
            .into_iter()
            .map(|p| hound::WavWriter::create(p, spec).unwrap())
            .collect::<Vec<_>>()
            .try_into()
            .unwrap_or_else(|_| {
                panic!("Failed to create hound::WavWriter array (probably wrong path)")
            });
        Self {
            size: Self::BUF_SIZE,
            inner_size: Self::BUF_SIZE_INNER,
            buf: vec![[0i32; Self::BUF_SIZE_INNER]; Self::BUF_SIZE],
            index: 0,
            inner_index: 0,
            filled: false,
            files,
        }
    }

    fn increment_index(&mut self) -> bool {
        let row_full = self.inner_index == Self::BUF_SIZE_INNER;
        if row_full {
            self.inner_index = 0;
            self.index += 1;
            self.index %= self.size;
        }
        row_full
    }

    fn push(&mut self, value: i32) -> bool {
        self.buf[self.index][self.inner_index] = value;
        self.increment_index()
    }

    fn get(&self, i: usize, j: usize) -> i32 {
        self.buf[self.index][self.inner_index]
    }

    fn compute_samples(&self) {
        for i in 0..=Self::BUF_SIZE_INNER {
            let mut j = Self::MID * i;
            let step = Self::MID - i;

            let mut sample = 0f64;

            for k in 0..8 {
                sample += f64::from(self.get(j, k)) / 8.0;
                j += step;
            }
        }
    }
}

#[allow(clippy::too_many_lines)]
fn make_wav_i2s<P: std::convert::AsRef<Path>>(
    from: DateTime<FixedOffset>,
    to: DateTime<FixedOffset>,
    path: P,
    dir: P,
) {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: 48000,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Int,
    };
    let mut writer = hound::WavWriter::create(path, spec).unwrap();
    let mut waves = std::fs::read_dir(dir)
        .unwrap()
        .flat_map(|f| f.map(|e| e.path()))
        .collect::<Vec<_>>();
    waves.sort_unstable();
    //eprintln!("{waves:?}");

    let from_nanos = from.timestamp_nanos_opt().unwrap();
    let to_nanos = to.timestamp_nanos_opt().unwrap();

    //eprintln!("{from_nanos} {to_nanos}");

    let mut best_pps = None;
    let mut best_diff = i64::MAX;

    for wav in &waves {
        let pps_vec = get_pps(wav);
        for p in &pps_vec {
            //eprintln!("{p:?}");
        }
        let best = pps_vec
            .into_iter()
            .min_by_key(|x| (x.nanos - from_nanos).abs());
        if let Some(best) = best {
            let diff = (best.nanos - from_nanos).abs();
            if diff < best_diff {
                best_pps = Some(best);
                best_diff = diff;
            }
        }
    }

    eprintln!("{best_pps:?}");

    if let Some(Pps { nanos, sample, file }) = best_pps {
        let mut nanos_diff = from_nanos - nanos;
        let mut backward = false;
        let mut start_sample = 0u32;
        let mut start_file = file.clone();
        let mut start_found = false;
        if nanos_diff < 0 {
            nanos_diff = -nanos_diff;
            backward = true;
        }
        #[allow(clippy::cast_precision_loss)]
        #[allow(clippy::cast_possible_truncation)]
        #[allow(clippy::cast_sign_loss)]
        let mut samples_diff = (nanos_diff as f64 / 1e9_f64 * 192_000.0).round() as u32;
        if backward {
            if sample >= samples_diff {
                start_sample = sample - samples_diff;
                start_found = true;
            } else {
                samples_diff -= sample;
                for wav in waves.iter().rev().skip_while(|x| **x != file).skip(1) {
                    let mut reader = hound::WavReader::open(wav).unwrap();
                    let wav_dur = reader.duration() / 4;
                    if samples_diff > wav_dur {
                        samples_diff -= wav_dur;
                    } else {
                        start_sample = wav_dur - samples_diff;
                        start_file.clone_from(wav);
                        start_found = true;
                        break;
                    }
                }
            }
        } else {
            let mut reader = hound::WavReader::open(file.clone()).unwrap();
            let wav_dur = reader.duration() / 4;
            if sample + samples_diff <= wav_dur {
                start_sample = sample + samples_diff;
                start_found = true;
            } else {
                samples_diff -= (wav_dur - sample);
                for wav in waves.iter().skip_while(|x| **x != file).skip(1) {
                    let mut reader = hound::WavReader::open(wav).unwrap();
                    let wav_dur = reader.duration() / 4;
                    if samples_diff > wav_dur {
                        samples_diff -= wav_dur;
                    } else {
                        start_sample = samples_diff;
                        start_file.clone_from(wav);
                        start_found = true;
                        break;
                    }
                }
            }
        }

        assert!(start_found, "Failed to find starting point");

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
            "[{{elapsed_precise}}] {{bar:40.cyan/blue}} {{pos:>{t}}}/{{len:{t}}} ({{percent}}%)"
        ))
            .unwrap()
            .progress_chars("##-"),
        );

        let mut start = true;
        let mut skip = 0;
        let mut end = false;
        for wav in waves.iter().skip_while(|x| **x != start_file) {
            let mut reader = hound::WavReader::open(wav).unwrap();
            if start {
                reader.seek(start_sample / 2).unwrap();
            }
            for s in reader.samples::<i32>() {
                let sample = s.unwrap();
                if start {
                    // TODO: find start
                    eprintln!("{sample:#x}");
                }
                #[allow(clippy::cast_possible_wrap)]
                if sample == 0xeeee_eeee_u32 as i32 {
                    skip += 3;
                } else if skip > 0 {
                    skip -= 1;
                } else {
                    // TODO: circular buffer insert here
                    // then calculate diags and output to files
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

        writer.finalize().unwrap();
        pb.finish();
    }
}

#[allow(clippy::too_many_lines)]
fn make_wav<P: std::convert::AsRef<Path>>(
    from: DateTime<FixedOffset>,
    to: DateTime<FixedOffset>,
    path: P,
    dir: P,
) {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: 48000,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Int,
    };
    let mut writer = hound::WavWriter::create(path, spec).unwrap();
    let mut waves = std::fs::read_dir(dir)
        .unwrap()
        .flat_map(|f| f.map(|e| e.path()))
        .collect::<Vec<_>>();
    waves.sort_unstable();
    //eprintln!("{waves:?}");

    let from_nanos = from.timestamp_nanos_opt().unwrap();
    let to_nanos = to.timestamp_nanos_opt().unwrap();

    //eprintln!("{from_nanos} {to_nanos}");

    let mut best_pps = None;
    let mut best_diff = i64::MAX;

    for wav in &waves {
        let pps_vec = get_pps(wav);
        for p in &pps_vec {
            //eprintln!("{p:?}");
        }
        let best = pps_vec
            .into_iter()
            .min_by_key(|x| (x.nanos - from_nanos).abs());
        if let Some(best) = best {
            let diff = (best.nanos - from_nanos).abs();
            if diff < best_diff {
                best_pps = Some(best);
                best_diff = diff;
            }
        }
    }

    eprintln!("{best_pps:?}");

    if let Some(Pps { nanos, sample, file }) = best_pps {
        let mut nanos_diff = from_nanos - nanos;
        let mut backward = false;
        let mut start_sample = 0u32;
        let mut start_file = file.clone();
        let mut start_found = false;
        if nanos_diff < 0 {
            nanos_diff = -nanos_diff;
            backward = true;
        }
        #[allow(clippy::cast_precision_loss)]
        #[allow(clippy::cast_possible_truncation)]
        #[allow(clippy::cast_sign_loss)]
        let mut samples_diff = (nanos_diff as f64 / 1e9_f64 * 48000.0).round() as u32;
        if backward {
            if sample >= samples_diff {
                start_sample = sample - samples_diff;
                start_found = true;
            } else {
                samples_diff -= sample;
                for wav in waves.iter().rev().skip_while(|x| **x != file).skip(1) {
                    let mut reader = hound::WavReader::open(wav).unwrap();
                    let wav_dur = reader.duration() / 2;
                    if samples_diff > wav_dur {
                        samples_diff -= wav_dur;
                    } else {
                        start_sample = wav_dur - samples_diff;
                        start_file.clone_from(wav);
                        start_found = true;
                        break;
                    }
                }
            }
        } else {
            let mut reader = hound::WavReader::open(file.clone()).unwrap();
            let wav_dur = reader.duration() / 2;
            if sample + samples_diff <= wav_dur {
                start_sample = sample + samples_diff;
                start_found = true;
            } else {
                samples_diff -= (wav_dur - sample);
                for wav in waves.iter().skip_while(|x| **x != file).skip(1) {
                    let mut reader = hound::WavReader::open(wav).unwrap();
                    let wav_dur = reader.duration() / 2;
                    if samples_diff > wav_dur {
                        samples_diff -= wav_dur;
                    } else {
                        start_sample = samples_diff;
                        start_file.clone_from(wav);
                        start_found = true;
                        break;
                    }
                }
            }
        }

        assert!(start_found, "Failed to find starting point");

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
            "[{{elapsed_precise}}] {{bar:40.cyan/blue}} {{pos:>{t}}}/{{len:{t}}} ({{percent}}%)"
        ))
            .unwrap()
            .progress_chars("##-"),
        );

        let mut start = true;
        let mut skip = 0;
        let mut end = false;
        for wav in waves.iter().skip_while(|x| **x != start_file) {
            let mut reader = hound::WavReader::open(wav).unwrap();
            if start {
                reader.seek(start_sample / 2).unwrap();
                start = false;
            }
            for s in reader.samples::<i32>() {
                let sample = s.unwrap();
                #[allow(clippy::cast_possible_wrap)]
                if sample == 0xeeee_eeee_u32 as i32 {
                    skip += 3;
                } else if skip > 0 {
                    skip -= 1;
                } else {
                    skip += 1;
                    writer.write_sample(sample);
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

        writer.finalize().unwrap();
        pb.finish();
    }
}

#[derive(Debug)]
struct Pps {
    nanos: i64,
    sample: u32,
    file: PathBuf,
}

fn get_pps(f: &PathBuf) -> Vec<Pps> {
    //println!("{f:?}");
    let mut reader = hound::WavReader::open(f).unwrap();
    let mut pps = false;
    let mut first_read = false;
    let mut prev = 0i32;
    let mut pps_vec = Vec::new();
    for (i, s) in reader.samples::<i32>().enumerate() {
        let sample = s.unwrap();
        #[allow(clippy::cast_possible_wrap)]
        if sample == 0xeeee_eeee_u32 as i32 {
            pps = true;
        } else if pps {
            if first_read {
                pps = false;
                first_read = false;
                let nanos = unsafe { transmute::<[i32; 2], i64>([sample, prev]) };
                #[allow(clippy::cast_possible_truncation)]
                pps_vec.push(Pps {
                    nanos,
                    sample: i as u32,
                    file: f.clone(),
                });
            } else {
                first_read = true;
                prev = sample;
            }
        }
    }
    pps_vec
}

fn main() {
    //println!("{}", chrono::Utc::now().timestamp_nanos_opt().unwrap());
    let args: Vec<String> = env::args().collect();
    let from = DateTime::parse_from_str(&args[1], "%Y-%m-%d %H:%M:%S%.3f %z").unwrap();
    let to = DateTime::parse_from_str(&args[2], "%Y-%m-%d %H:%M:%S%.3f %z").unwrap();
    let path = &args[3];
    let dir = &args[4];
    make_wav(from, to, path, dir);
    //let mut reader = hound::WavReader::open(args[1].clone()).unwrap();
    //println!("{:?}", reader.spec());
    //let mut pps = false;
    //let mut first_read = false;
    ////let mut read = 0u8;
    //let mut prev = 0i32;
    //let mut diff = 0u64;
    //reader
    //    .seek(85000 / u32::from(reader.spec().channels))
    //    .unwrap();
    //for s in reader.samples::<i32>() {
    //    let sample = s.unwrap();
    //    #[allow(clippy::cast_possible_wrap)]
    //    if sample == 0xeeee_eeee_u32 as i32 {
    //        pps = true;
    //    } else if pps {
    //        //if read == 0 {
    //        //    prev = sample;
    //        //    read = 1;
    //        //} else if read == 1 {
    //        //    let nanos = unsafe { transmute::<[i32; 2], i64>([sample, prev]) };
    //        //    let dt = DateTime::from_timestamp_nanos(nanos);
    //        //    println!("{diff}");
    //        //    diff = 0;
    //        //    println!("{dt}");
    //        //    read = 2;
    //        //} else if read == 2 {
    //        //    prev = sample;
    //        //    read = 3;
    //        //} else {
    //        //    let nanos = unsafe { transmute::<[i32; 2], i64>([sample, prev]) };
    //        //    let dt = DateTime::from_timestamp_nanos(nanos);
    //        //    println!("{dt}");
    //        //    read = 0;
    //        //    pps = false;
    //        //}
    //        if first_read {
    //            pps = false;
    //            first_read = false;
    //            let nanos = unsafe { transmute::<[i32; 2], i64>([sample, prev]) };
    //            let dt = DateTime::from_timestamp_nanos(nanos);
    //            println!("{diff}");
    //            diff = 0;
    //            println!("{dt}");
    //        } else {
    //            first_read = true;
    //            prev = sample;
    //        }
    //    } else {
    //        diff += 1;
    //    }
    //}
}
