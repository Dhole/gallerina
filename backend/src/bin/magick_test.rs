// use backend::magick::make_thumb;
// use backend::ffmpeg::make_thumb;
use std::env;
use std::fs::File;
use std::io::prelude::*;

fn main() {
    let path = env::args().nth(1).unwrap();
    // for i in 0..10 {
    //     let thumb = make_thumb(&path).unwrap();
    //     println!("{} {}", i, thumb[0]);
    // }
    let mut file = File::open(path).unwrap();
    let mut buf = Vec::new();
    file.read_to_end(&mut buf).unwrap();
    backend::magick::convert_to_webp(&buf).unwrap();
    println!("{}", buf[0]);
}
