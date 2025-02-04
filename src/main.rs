//#![allow(unused)]
use std::env;

use chrono::DateTime;

mod i2s;
mod pps;
mod umc;

fn main() {
    let args: Vec<String> = env::args().collect();
    let from = DateTime::parse_from_str(&args[1], "%Y-%m-%d %H:%M:%S%.3f %z").unwrap();
    let to = DateTime::parse_from_str(&args[2], "%Y-%m-%d %H:%M:%S%.3f %z").unwrap();
    let output = &args[3];
    let input_dir = &args[4];

    let mode = &args[5];
    if mode == "umc" {
        umc::make_wav(from, to, output, input_dir);
    } else if mode == "i2s" {
        i2s::make_wav(from, to, output, input_dir);
    } else {
        eprintln!("Last argument should be either \"umc\" or \"i2s\"");
    }
}
