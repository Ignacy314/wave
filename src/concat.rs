use std::path::Path;

pub fn concat<P: std::convert::AsRef<Path>>(input_dir: P, output: P, step: usize) {
    let mut waves = std::fs::read_dir(input_dir.as_ref())
        .unwrap()
        .flat_map(|f| f.map(|e| e.path()))
        .collect::<Vec<_>>();
    waves.sort_unstable();

    let file = hound::WavReader::open(&waves[0]).unwrap();
    let spec = file.spec();

    let nanos = &waves[0]
        .file_stem()
        .unwrap()
        .to_str()
        .unwrap()
        .parse::<i64>()
        .unwrap();

    let start = chrono::DateTime::from_timestamp_nanos(*nanos);

    let output_path = output.as_ref().parent().unwrap().to_str().unwrap();
    let output_stem = output.as_ref().file_stem().unwrap().to_str().unwrap();
    let output_ext = output.as_ref().extension().unwrap().to_str().unwrap();

    let mut writer = hound::WavWriter::create(
        format!("{output_path}/{output_stem}_{}.{output_ext}", start.to_rfc3339()),
        spec,
    )
    .unwrap();

    for wav in waves.iter() {
        let mut reader = match hound::WavReader::open(wav) {
            Ok(r) => r,
            Err(_e) => {
                continue;
            }
        };

        reader.samples::<i32>().step_by(step).for_each(|s| {
            writer.write_sample(s.unwrap()).unwrap();
        });
    }
}
