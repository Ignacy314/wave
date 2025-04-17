// use std::{fs::DirEntry, path::Path};
//
// pub fn concat<P: std::convert::AsRef<Path>>(input_dir: P, output_dir: P) {
//     let flights = std::fs::read_dir(input_dir.as_ref().join("umc")).unwrap();
//
//     for module in 4..=13 {
//         let module_str = module.to_string();
//         let wavs = flights
//             .map(|f| f.unwrap().path().join(&module_str))
//             .flat_map(|p| std::fs::read_dir(p).unwrap().map(|d| d.unwrap()));
//         let csvs =
//             std::fs::read_dir(input_dir.as_ref().join("module_csvs").join(&module_str)).unwrap();
//
//         for wav in wavs {
//
//         }
//     }
//
//     let mut waves = std::fs::read_dir(input_dir.as_ref())
//         .unwrap()
//         .flat_map(|f| f.map(|e| e.path()))
//         .collect::<Vec<_>>();
//     waves.sort_unstable();
//
//     let spec = hound::WavSpec {
//         channels: 1,
//         sample_rate: 48000,
//         bits_per_sample: 32,
//         sample_format: hound::SampleFormat::Int,
//     };
//
//     let clock_start_nanos_str = clock.as_ref().file_stem().unwrap().to_str().unwrap();
//
//     let mut wav_iter = waves.iter().peekable();
//     while let Some(wav) = wav_iter.peek() {
//         if wav.file_stem().unwrap().to_str().unwrap() == clock_start_nanos_str {
//             break;
//         }
//         wav_iter.next();
//     }
//
//     if wav_iter.peek().is_none() {
//         eprintln!("Clock start not found");
//         return;
//     }
//
//     let mut start_nanos = clock_start_nanos_str.parse::<i64>().unwrap();
//     let mut end_file = "".to_owned();
//     let mut reader = csv::Reader::from_path(clock).unwrap();
//     if let Some(record) = reader.deserialize().next() {
//         let r: Record = record.unwrap();
//         start_nanos = r.time - (r.sample as f64 / 48000.0 * 1e9).round() as i64;
//     }
//     if let Some(Ok(Record { file, .. })) = reader.deserialize().last() {
//         end_file = file;
//     }
//
//     let start = chrono::DateTime::from_timestamp_nanos(start_nanos);
//
//     let output_path = output.as_ref().parent().unwrap().to_str().unwrap();
//     let output_stem = output.as_ref().file_stem().unwrap().to_str().unwrap();
//     let output_ext = output.as_ref().extension().unwrap().to_str().unwrap();
//
//     let mut writer = hound::WavWriter::create(
//         format!("{output_path}/{output_stem}_{}.{output_ext}", start.to_rfc3339()),
//         spec,
//     )
//     .unwrap();
//
//     for wav in wav_iter {
//         let mut reader = match hound::WavReader::open(wav) {
//             Ok(r) => r,
//             Err(_e) => {
//                 continue;
//             }
//         };
//
//         reader.samples::<i32>().step_by(step).for_each(|s| {
//             writer.write_sample(s.unwrap()).unwrap();
//         });
//
//         if wav.to_str().unwrap() == end_file {
//             break;
//         }
//     }
// }
