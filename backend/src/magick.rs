use magick_rust::{magick_wand_genesis, MagickWand};
use std::sync::Once;

use crate::scanner::{THUMB_QUALITY, THUMB_SIZE};

// Used to make sure MagickWand is initialized exactly once. Note that we
// do not bother shutting down, we simply exit when we're done.
static START: Once = Once::new();

pub fn make_thumb(path: &str) -> Result<Vec<u8>, &'static str> {
    START.call_once(|| {
        magick_wand_genesis();
    });

    let mut wand = MagickWand::new();
    wand.read_image(path)?;
    wand.set_image_compression_quality(THUMB_QUALITY as usize)?;
    wand.thumbnail_image(THUMB_SIZE as usize, THUMB_SIZE as usize);
    wand.write_image_blob("jpeg")
}
