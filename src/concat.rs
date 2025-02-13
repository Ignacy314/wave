use std::path::Path;

pub fn concat<P: std::convert::AsRef<Path>>(input_dir: P, output: P) {
    let mut waves = std::fs::read_dir(input_dir.as_ref())
        .unwrap()
        .flat_map(|f| f.map(|e| e.path()))
        .collect::<Vec<_>>();
    waves.sort_unstable();

    let file = hound::WavReader::open(&waves[0]).unwrap();
    let spec = file.spec();

    let mut writer = hound::WavWriter::create(output, spec).unwrap();

    for wav in waves.iter() {
        let mut reader = match hound::WavReader::open(wav) {
            Ok(r) => r,
            Err(_e) => {
                continue;
            }
        };

        reader.samples::<i32>().for_each(|s| {
            writer.write_sample(s.unwrap()).unwrap();
        });
    }
}
