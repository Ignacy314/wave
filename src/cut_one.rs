use std::path::Path;

use indicatif::{ProgressBar, ProgressStyle};

pub fn make_wav<P: std::convert::AsRef<Path>>(output: P, input: P, start: u32, samples: u64) {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: 48000,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Int,
    };

    let mut samples = samples;

    let mut reader = hound::WavReader::open(input).unwrap();
    let mut writer = hound::WavWriter::create(output, spec).unwrap();

    reader.seek(start).unwrap();

    let pb = ProgressBar::new(samples);
    let t = (samples as f64).log10().ceil() as u64;
    pb.set_style(
        ProgressStyle::with_template(&format!(
            "[{{elapsed_precise}}] {{bar:40.cyan/blue}} {{pos:>{t}}}/{{len:{t}}} ({{percent}}%) {{msg}}"
        ))
        .unwrap()
        .progress_chars("##-"),
    );

    for s in reader.samples::<i32>() {
        if samples != 0 {
            writer.write_sample(s.unwrap()).unwrap();
            samples -= 1;
            pb.inc(1);
        } else {
            break;
        }
    }

    let samples_processed = pb.position();
    pb.finish_with_message(format!("Samples processed: {samples_processed}"));
}
