use std::env;
use std::mem::transmute;

use chrono::DateTime;

fn main() {
    let args: Vec<String> = env::args().collect();
    let mut reader = hound::WavReader::open(args[1].clone()).unwrap();
    println!("{:?}", reader.spec());
    let mut pps = false;
    let mut first_read = false;
    let mut prev = 0i32;
    let mut diff = 0u64;
    for s in reader.samples::<i32>() {
        let sample = s.unwrap();
        #[allow(clippy::cast_possible_wrap)]
        if sample == 0xeeee_eeee_u32 as i32 {
            pps = true;
        } else if pps {
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
