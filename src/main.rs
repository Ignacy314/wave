use std::env;
use std::mem::transmute;
use std::path::Path;

use chrono::{DateTime, Utc};

fn make_wav<P: std::convert::AsRef<Path>>(from: DateTime<Utc>, to: DateTime<Utc>, path: P, dir: P) {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: 48000,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Int,
    };
    let mut writer = hound::WavWriter::create(path, spec).unwrap();
    let waves = std::fs::read_dir(dir).unwrap();

    for wav in waves.flatten() {

    }
}

fn main() {
    //make_wav(DateTime::UNIX_EPOCH, DateTime::UNIX_EPOCH, "a");
    let args: Vec<String> = env::args().collect();
    let mut reader = hound::WavReader::open(args[1].clone()).unwrap();
    println!("{:?}", reader.spec());
    let mut pps = false;
    let mut first_read = false;
    //let mut read = 0u8;
    let mut prev = 0i32;
    let mut diff = 0u64;
    reader.seek(85000 / u32::from(reader.spec().channels)).unwrap();
    for s in reader.samples::<i32>() {
        let sample = s.unwrap();
        #[allow(clippy::cast_possible_wrap)]
        if sample == 0xeeee_eeee_u32 as i32 {
            pps = true;
        } else if pps {
            //if read == 0 {
            //    prev = sample;
            //    read = 1;
            //} else if read == 1 {
            //    let nanos = unsafe { transmute::<[i32; 2], i64>([sample, prev]) };
            //    let dt = DateTime::from_timestamp_nanos(nanos);
            //    println!("{diff}");
            //    diff = 0;
            //    println!("{dt}");
            //    read = 2;
            //} else if read == 2 {
            //    prev = sample;
            //    read = 3;
            //} else {
            //    let nanos = unsafe { transmute::<[i32; 2], i64>([sample, prev]) };
            //    let dt = DateTime::from_timestamp_nanos(nanos);
            //    println!("{dt}");
            //    read = 0;
            //    pps = false;
            //}
            if first_read {
                pps = false;
                first_read = false;
                let nanos = unsafe { transmute::<[i32; 2], i64>([sample, prev]) };
                let dt = DateTime::from_timestamp_nanos(nanos);
                println!("{diff}");
                diff = 0;
                println!("{dt}");
            } else {
                first_read = true;
                prev = sample;
            }
        } else {
            diff += 1;
        }
    }
}
