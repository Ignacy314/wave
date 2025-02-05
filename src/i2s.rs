use std::fs::File;
use std::io::BufWriter;
use std::path::Path;

use chrono::{DateTime, FixedOffset};
use indicatif::{ProgressBar, ProgressStyle};

use crate::pps::{find_best, find_start, Pps};

struct CircularI2S {
    _size: usize,
    _inner_size: usize,
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

    fn new<P: std::convert::AsRef<Path>>(path: P, num: u8) -> Self {
        let paths: [String; Self::BUF_SIZE_INNER + 1] = (0..=Self::BUF_SIZE_INNER)
            .map(|i| format!("{}_{num}_{i}.wav", path.as_ref().display()))
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
            _size: Self::BUF_SIZE,
            _inner_size: Self::BUF_SIZE_INNER,
            buf: vec![[0i32; Self::BUF_SIZE_INNER]; Self::BUF_SIZE],
            index: 0,
            inner_index: 0,
            filled: false,
            files,
        }
    }

    fn increment_index(&mut self) -> bool {
        let row_full = self.inner_index == Self::BUF_SIZE_INNER - 1;
        if row_full {
            self.inner_index = 0;
            if self.index == Self::BUF_SIZE - 1 {
                self.filled = true;
                self.index = 0;
            } else {
                self.index += 1;
            }
        } else {
            self.inner_index += 1;
        }
        row_full
    }

    fn set_inner(&mut self, value: i32, index: usize) -> bool {
        self.buf[self.index][index] = value;
        self.increment_index()
    }

    //fn push(&mut self, value: i32) -> bool {
    //    self.buf[self.index][self.inner_index] = value;
    //    self.increment_index()
    //}

    fn get(&self, i: usize, j: usize) -> i32 {
        self.buf[i][j]
    }

    fn compute_samples(&mut self) {
        if self.filled {
            for i in 0..=Self::BUF_SIZE_INNER {
                let mut j = Self::MID * i;
                let step = Self::MID - i;

                let mut sample = 0f64;

                for k in 0..8 {
                    sample += f64::from(self.get(j, k)) / 8.0;
                    j += step;
                }

                #[allow(clippy::cast_possible_truncation)]
                let sample = sample as i32;
                self.files[i].write_sample(sample).unwrap();
            }
        }
    }

    fn finalize(self) {
        self.files.into_iter().for_each(|w| w.finalize().unwrap());
    }
}

#[allow(clippy::too_many_lines)]
pub fn make_wav<P: std::convert::AsRef<Path>>(
    timestamps: Option<(DateTime<FixedOffset>, DateTime<FixedOffset>)>,
    path: P,
    dir: P,
) {
    let mut bufs = [
        CircularI2S::new(path.as_ref(), 1),
        CircularI2S::new(path, 2),
    ];

    let (from_nanos, to_nanos) = if let Some(timestamps) = timestamps {
        (
            timestamps.0.timestamp_nanos_opt().unwrap(),
            timestamps.1.timestamp_nanos_opt().unwrap(),
        )
    } else {
        (0, 0)
    };

    //let from_nanos = from.timestamp_nanos_opt().unwrap();
    //let to_nanos = to.timestamp_nanos_opt().unwrap();

    let (best_pps, mut _best_diff, waves) = find_best(dir.as_ref(), from_nanos);

    eprintln!("{best_pps:?}");

    let channels = 4;

    if let Some(Pps { nanos, sample, file }) = best_pps {
        let (start_file, start_sample) =
            find_start(from_nanos, nanos, sample, &file, &waves, channels);

        #[allow(clippy::cast_precision_loss)]
        #[allow(clippy::cast_possible_truncation)]
        #[allow(clippy::cast_sign_loss)]
        let mut samples_left =
            [((to_nanos - from_nanos) as f64 / 1e9_f64 * 48000.0).round() as u32; 2];

        #[allow(clippy::cast_possible_truncation)]
        let n = samples_left[0] * samples_left.len() as u32;
        let pb = ProgressBar::new(u64::from(n));
        #[allow(clippy::cast_possible_truncation)]
        #[allow(clippy::cast_sign_loss)]
        let t = f64::from(n).log10().ceil() as u64;
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
            let mut reader = match hound::WavReader::open(wav) {
                Ok(r) => r,
                Err(_e) => {
                    continue;
                }
            };
            if start {
                reader.seek(start_sample / channels).unwrap();
            }
            for s in reader.samples::<i32>() {
                let sample = s.unwrap();
                if start {
                    #[allow(clippy::cast_sign_loss)]
                    let mic = ((sample as u32 & 0b1000) >> 3) as usize;
                    #[allow(clippy::cast_sign_loss)]
                    let inner_index = (sample as u32 & 0b111) as usize;
                    if mic != 0 || inner_index != 0 {
                        continue;
                    }
                    start = false;
                }
                #[allow(clippy::cast_possible_wrap)]
                if skip > 0 {
                    skip -= 1;
                } else if sample == 0xeeee_eeee_u32 as i32 {
                    skip += 3;
                } else {
                    #[allow(clippy::cast_sign_loss)]
                    let mic = ((sample as u32 & 0b1000) >> 3) as usize;

                    #[allow(clippy::cast_sign_loss)]
                    let inner_index = (sample as u32 & 0b111) as usize;

                    if samples_left[mic] > 0 && bufs[mic].set_inner(sample, inner_index) {
                        bufs[mic].compute_samples();
                        samples_left[mic] -= 1;
                        pb.inc(1);
                    }

                    if samples_left.iter().all(|x| *x == 0) {
                        end = true;
                        break;
                    }
                }
            }
            if end {
                break;
            }
        }

        for b in bufs {
            b.finalize();
        }
        pb.finish();
    }
}
