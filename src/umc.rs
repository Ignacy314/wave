use std::path::Path;

use chrono::{DateTime, FixedOffset};
use indicatif::{ProgressBar, ProgressStyle};

use crate::pps::{find_best, find_start, Pps};

#[allow(clippy::too_many_lines)]
pub fn make_wav<P: std::convert::AsRef<Path>>(
    from: DateTime<FixedOffset>,
    to: DateTime<FixedOffset>,
    output: P,
    input_dir: P,
) {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: 48000,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Int,
    };
    let mut writer =
        hound::WavWriter::create(format!("{}.wav", output.as_ref().to_str().unwrap()), spec)
            .unwrap();

    let from_nanos = from.timestamp_nanos_opt().unwrap();
    let to_nanos = to.timestamp_nanos_opt().unwrap();

    let (best_pps, mut _best_diff, waves) = find_best(input_dir.as_ref(), from_nanos);

    eprintln!("{best_pps:?}");

    let channels = 2;

    if let Some(Pps { nanos, sample, file }) = best_pps {
        let (start_file, start_sample) =
            find_start(from_nanos, nanos, sample, &file, &waves, channels);

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
                if skip > 0 {
                    skip -= 1;
                } else if sample == 0xeeee_eeee_u32 as i32 {
                    skip += 3;
                } else {
                    skip += 1;
                    writer.write_sample(sample).unwrap();
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
