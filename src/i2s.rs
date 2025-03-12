use std::fs::File;
use std::io::BufWriter;
use std::path::Path;

use circular_buffer::CircularBuffer;
use indicatif::{ProgressBar, ProgressStyle};

use crate::Record;

//const CHANNELS: u32 = 4;
const FREQ: f64 = 192000.0;
const BUF_SIZE: usize = 33;
const BUF_SIZE_INNER: usize = 8;
const MID: usize = BUF_SIZE_INNER / 2;

struct CircularI2S {
    _size: usize,
    _inner_size: usize,
    buf: CircularBuffer<BUF_SIZE, [i32; BUF_SIZE_INNER]>,
    new_row: [i32; BUF_SIZE_INNER],
    //index: usize,
    inner_index: usize,
    files: [hound::WavWriter<BufWriter<File>>; BUF_SIZE_INNER + 1],
}

impl CircularI2S {
    fn new<P: std::convert::AsRef<Path>>(path: P, num: u8) -> Self {
        let paths: [String; BUF_SIZE_INNER + 1] = (0..=BUF_SIZE_INNER)
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
        let files: [hound::WavWriter<BufWriter<File>>; BUF_SIZE_INNER + 1] = paths
            .into_iter()
            .map(|p| hound::WavWriter::create(p, spec).unwrap())
            .collect::<Vec<_>>()
            .try_into()
            .unwrap_or_else(|_| {
                panic!("Failed to create hound::WavWriter array (probably wrong path)")
            });
        Self {
            _size: BUF_SIZE,
            _inner_size: BUF_SIZE_INNER,
            buf: CircularBuffer::new(),
            new_row: [0; BUF_SIZE_INNER],
            //index: 0,
            inner_index: 0,
            files,
        }
    }

    fn increment_index(&mut self) -> bool {
        let row_full = self.inner_index == BUF_SIZE_INNER - 1;
        if row_full {
            self.buf.push_back(self.new_row);
            self.inner_index = 0;
            //if self.index == BUF_SIZE - 1 {
            //    self.index = 0;
            //} else {
            //    self.index += 1;
            //}
        } else {
            self.inner_index += 1;
        }
        row_full
    }

    fn set_inner(&mut self, value: i32, index: usize) -> bool {
        self.new_row[index] = value;
        self.increment_index()
    }

    //fn push(&mut self, value: i32) -> bool {
    //    self.buf[self.index][self.inner_index] = value;
    //    self.increment_index()
    //}

    //fn get(&self, i: usize, j: usize) -> i32 {
    //    self.buf[i][j]
    //}

    fn compute_samples(&mut self) {
        if self.buf.is_full() {
            for i in 0..=BUF_SIZE_INNER {
                let mut j = MID * i;
                let step = MID - i;

                let mut sample = 0;

                for k in 0..8 {
                    sample += self.buf[j][k] as i64;
                    j += step;
                }

                let sample = (sample / 8) as i32;
                self.files[i].write_sample(sample).unwrap();
            }
        }
    }

    fn finalize(self) {
        self.files.into_iter().for_each(|w| w.finalize().unwrap());
    }
}

pub fn make_wav<P: std::convert::AsRef<Path>>(
    output: P,
    input_dir: P,
    clock: P,
    start: Option<i64>,
    samples: Option<u64>,
) {
    let mut bufs = [
        CircularI2S::new(output.as_ref(), 1),
        CircularI2S::new(output.as_ref(), 2),
    ];

    let mut waves = std::fs::read_dir(input_dir.as_ref())
        .unwrap()
        .flat_map(|f| f.map(|e| e.path()))
        .collect::<Vec<_>>();
    waves.sort_unstable();

    let clock_start_nanos_str = clock.as_ref().file_stem().unwrap().to_str().unwrap();

    eprintln!("Go to clock start");
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
    let file_start_sample = file_start_sample as u32;

    eprintln!("Go to time start");
    while let Some(wav) = wav_iter.peek() {
        if **wav == start_file {
            break;
        }
        wav_iter.next();
    }
    eprintln!("After time start");

    //let start_nanos = if let Some(start) = start {
    //    start
    //} else {
    //    records[0].time - (records[0].sample as f64 / FREQ * 1e9).round() as i64
    //};
    let end_file = records[n_records - 1].file.clone();
    let end_file = input_dir.as_ref().join(end_file);

    let mut samples = if let Some(samples) = samples {
        [samples; 2]
    } else {
        let samples = records[n_records - 1].sample;
        [samples; 2]
    };


    eprintln!("pb");
    let pb = ProgressBar::new(samples[0] * 2);
    let t = (2.0 * samples[0] as f64).log10().ceil() as u64;
    pb.set_style(
        ProgressStyle::with_template(&format!(
            "[{{elapsed_precise}}] {{bar:40.cyan/blue}} {{pos:>{t}}}/{{len:{t}}} ({{percent}}%) {{msg}}"
        ))
        .unwrap()
        .progress_chars("##-"),
    );

    let mut med = Vec::new();

    eprintln!("start");
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
            } else {
                continue;
            }
        }

        for s in reader.samples::<i32>() {
            let sample = s.unwrap();
            med.push(sample);
            if start {
                let mic = ((sample as u32 & 0b1000) >> 3) as usize;
                let inner_index = (sample as u32 & 0b111) as usize;
                if mic != 1 || inner_index != 1 {
                    continue;
                }
                start = false;
            }
            let mic = ((sample as u32 & 0b1000) >> 3) as usize;

            let inner_index = (sample as u32 & 0b111) as usize;

            if samples[mic] > 0 && bufs[mic].set_inner(sample, inner_index) {
                bufs[mic].compute_samples();
                samples[mic] -= 1;
                pb.inc(1);
            }

            if samples.iter().all(|x| *x == 0) {
                end = true;
                break;
            }
        }

        if end || *wav == end_file {
            eprintln!("{end}");
            break;
        }
    }
    med.sort_unstable();
    let med = med[med.len() / 2];
    for b in bufs {
        b.finalize();
    }
    let samples_processed = pb.position();
    pb.finish_with_message(format!("Samples processed: {samples_processed} | Median: {med}"));
}
