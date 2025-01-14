use std::env;

use chrono::DateTime;

fn main() {
    let args: Vec<String> = env::args().collect();
    let mut reader = hound::WavReader::open(args[1].clone()).unwrap();
    println!("{:?}", reader.spec());
    let mut pps = false;
    let mut first_read = false;
    let mut nanos = 0i64;
    for s in reader.samples::<i32>() {
        let sample = s.unwrap();
        #[allow(clippy::cast_possible_wrap)]
        if sample == 0xeeee_eeee_u32 as i32 {
            pps = true;
        } else if pps {
            if first_read {
                pps = false;
                first_read = false;
                nanos += i64::from(sample);
            } else {
                first_read = true;
                nanos = i64::from(sample) << 32;
            }

            let dt = DateTime::from_timestamp_nanos(nanos);
            eprintln!("{dt}");
        }
    }
}
