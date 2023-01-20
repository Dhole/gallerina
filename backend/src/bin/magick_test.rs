// use backend::magick::make_thumb;
use backend::ffmpeg::make_thumb;
use std::env;

fn main() {
    let path = env::args().nth(1).unwrap();
    for i in 0..10 {
        let thumb = make_thumb(&path).unwrap();
        println!("{} {}", i, thumb[0]);
    }
}
