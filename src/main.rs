use std::env;

fn main() {
    let args: Vec<String> = env::args().collect();
    let reader = hound::WavReader::open(args[1].clone()).unwrap();
    println!("{:?}", reader.spec());
}
