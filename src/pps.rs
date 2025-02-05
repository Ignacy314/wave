use std::mem::transmute;
use std::path::{Path, PathBuf};

use indicatif::{ProgressBar, ProgressStyle};

#[derive(Debug)]
pub struct Pps {
    pub nanos: i64,
    pub sample: u32,
    pub file: PathBuf,
}

fn get_pps(f: &PathBuf) -> Vec<Pps> {
    let mut pps_vec = Vec::new();
    let mut reader = match hound::WavReader::open(f) {
        Ok(r) => r,
        Err(_e) => {
            return pps_vec;
        }
    };
    let mut pps = false;
    let mut first_read = false;
    let mut prev = 0i32;
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
                    sample: (i - 2) as u32,
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

pub fn find_best(dir: &Path, from_nanos: i64) -> (Option<Pps>, i64, Vec<PathBuf>) {
    let start_nanos = from_nanos - 1_000_000_000 * 60;
    let mut waves = std::fs::read_dir(dir)
        .unwrap()
        .flat_map(|f| f.map(|e| e.path()))
        .filter(|f| {
            if let Some(ext) = f.extension() {
                if !ext.eq("wav") {
                    return false;
                }
            } else {
                return false;
            }
            let str = f.file_name().unwrap().to_str().unwrap();
            let str = &str[..str.len() - 4];
            let nanos = str.parse::<i64>().unwrap();
            nanos >= start_nanos
        })
        .collect::<Vec<_>>();
    waves.sort_unstable();

    let mut best_pps = None;
    let mut best_diff = i64::MAX;

    let n = waves.len() as u64;
    let pb = ProgressBar::new(n);
    #[allow(clippy::cast_possible_truncation)]
    #[allow(clippy::cast_sign_loss)]
    let t = f64::from(n as u32).log10().ceil() as u64;
    pb.set_style(
        ProgressStyle::with_template(&format!(
            "[{{elapsed_precise}}] {{bar:40.cyan/blue}} {{pos:>{t}}}/{{len:{t}}} ({{percent}}%)"
        ))
        .unwrap()
        .progress_chars("##-"),
    );

    for (i, wav) in waves.iter().enumerate() {
        let pps_vec = get_pps(wav);

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

        pb.inc(1);

        if i >= 5 && best_diff <= 500_000_000 {
            break;
        }
        if i >= 15 && best_diff <= 60_000_000_000 {
            break;
        }
    }

    pb.finish();
    (best_pps, best_diff, waves)
}

pub fn find_start(
    from_nanos: i64,
    nanos: i64,
    sample: u32,
    file: &PathBuf,
    waves: &[PathBuf],
    channels: u32,
    freq: f64,
) -> (PathBuf, u32) {
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
    let mut samples_diff = (nanos_diff as f64 / 1e9_f64 * freq).round() as u32;
    if backward {
        if sample >= samples_diff {
            start_sample = sample - samples_diff;
            start_found = true;
        } else {
            samples_diff -= sample;
            for wav in waves.iter().rev().skip_while(|x| *x != file).skip(1) {
                let reader = match hound::WavReader::open(wav) {
                    Ok(r) => r,
                    Err(_e) => {
                        continue;
                    }
                };
                let wav_dur = reader.duration() / channels;
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
        let reader = hound::WavReader::open(file.clone()).unwrap();
        let wav_dur = reader.duration() / channels;
        if sample + samples_diff <= wav_dur {
            start_sample = sample + samples_diff;
            start_found = true;
        } else {
            samples_diff -= wav_dur - sample;
            for wav in waves.iter().skip_while(|x| *x != file).skip(1) {
                let reader = match hound::WavReader::open(wav) {
                    Ok(r) => r,
                    Err(_e) => {
                        continue;
                    }
                };
                let wav_dur = reader.duration() / channels;
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

    (start_file, start_sample)
}
