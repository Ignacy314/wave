use std::env;
use std::fs::File;
use std::io::Read;
use std::mem::transmute;

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
                nanos = unsafe { transmute::<[i32; 2], i64>([sample, (nanos >> 32) as i32]) };
                let dt = DateTime::from_timestamp_nanos(nanos);
                eprintln!("{dt}");
            } else {
                first_read = true;
                nanos = i64::from(sample) << 32;
            }
        }
    }
    //let mut file = File::open(args[1].clone()).unwrap();
    //let mut buf = [0u8; 4];
    //let mut pps = false;
    //let mut first_read = false;
    //let mut nanos = 0i64;
    //while let Ok(s) = file.read(&mut buf) {
    //    if s == 0 {
    //        break;
    //    }
    //
    //    let sample: i32 = i32::from_ne_bytes(buf);
    //    #[allow(clippy::cast_possible_wrap)]
    //    if sample == 0xeeee_eeee_u32 as i32 {
    //        pps = true;
    //    } else if pps {
    //        println!("{sample:#x}");
    //        if first_read {
    //            pps = false;
    //            first_read = false;
    //            println!("{nanos:#x}");
    //            println!("{:#x}", i64::from(sample) & 0x1111_1111);
    //            nanos = unsafe { transmute([sample, (nanos >> 32) as i32])};
    //            //nanos += i64::from(sample) & 0x1111_1111;
    //            println!("{nanos:#x}");
    //            let dt = DateTime::from_timestamp_nanos(nanos);
    //            eprintln!("{dt}");
    //        } else {
    //            first_read = true;
    //            nanos = i64::from(sample) << 32;
    //        }
    //    }
    //}
}
