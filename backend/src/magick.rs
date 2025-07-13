use magick_rust::{magick_wand_genesis, MagickError, MagickWand};
use std::sync::Once;

use crate::scanner::{THUMB_QUALITY, THUMB_SIZE};

// Used to make sure MagickWand is initialized exactly once. Note that we
// do not bother shutting down, we simply exit when we're done.
static START: Once = Once::new();

pub fn make_thumb(path: &str) -> Result<Vec<u8>, MagickError> {
    START.call_once(|| {
        magick_wand_genesis();
    });

    let mut wand = MagickWand::new();
    wand.read_image(path)?;
    let (mut width, mut height) = (wand.get_image_width(), wand.get_image_height());
    if width == 0 {
        log::warn!("width=0 in {}", path);
        width = 16;
    }
    if height == 0 {
        log::warn!("width=0 in {}", path);
        height = 16;
    }
    let width_ratio = THUMB_SIZE as f64 / width as f64;
    let height_ratio = THUMB_SIZE as f64 / height as f64;
    let (mut new_width, mut new_height) = if width_ratio < height_ratio {
        (THUMB_SIZE as usize, (height as f64 * width_ratio) as usize)
    } else {
        ((width as f64 * height_ratio) as usize, THUMB_SIZE as usize)
    };
    if new_width == 0 || new_height == 0 {
        log::warn!(
            "Invalid thumb size {:?} from {:?} in {}",
            (width, height),
            (new_width, new_height),
            path
        );
        new_width = 16;
        new_height = 16;
    }
    wand.set_image_compression_quality(THUMB_QUALITY as usize)?;

    let orientation = wand.get_image_orientation();
    let (new_width, new_height) = match orientation {
        1 => (new_width, new_height),
        o => {
            wand.auto_orient();
            match o {
                6 | 8 | 5 | 7 => (new_height, new_width),
                _ => (new_width, new_height),
            }
        }
    };
    wand.thumbnail_image(new_width, new_height);
    wand.write_image_blob("webp")
}

pub fn convert_to_webp(
    buf: &[u8],
    quality: usize,
    compression: usize,
) -> Result<Vec<u8>, MagickError> {
    START.call_once(|| {
        magick_wand_genesis();
    });

    let mut wand = MagickWand::new();
    wand.read_image_blob(buf)?;
    wand.set_image_compression_quality(quality)?;
    wand.set_option("webp:method", compression.to_string().as_str())?;
    wand.auto_orient();
    // let orientation = wand.get_image_orientation();
    // let (new_width, new_height) = match orientation {
    //     1 => (new_width, new_height),
    //     o => {
    //         wand.auto_orient();
    //         match o {
    //             6 | 8 | 5 | 7 => (new_height, new_width),
    //             _ => (new_width, new_height),
    //         }
    //     }
    // };
    wand.write_image_blob("webp")
}
