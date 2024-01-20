use std::process::Command;

use crate::scanner::{ThumbError, THUMB_SIZE};

pub fn make_thumb(path: &str) -> Result<Vec<u8>, ThumbError> {
    // https://ffmpeg.org/ffmpeg-all.html#thumbnail
    // ffmpeg -i in.avi -vf thumbnail,scale=300:200 -frames:v 1 out.png
    let vf_arg = format!(
        "thumbnail,scale='if(gt(iw,ih),{},trunc(oh*a/2)*2)':'if(gt(iw,ih),trunc(ow/a/2)*2,{})'",
        THUMB_SIZE, THUMB_SIZE
    );
    let output = Command::new("ffmpeg")
        .arg("-v")
        .arg("error")
        .arg("-i")
        .arg(path)
        .arg("-vf")
        .arg(vf_arg.as_str())
        .arg("-frames:v")
        .arg("1")
        .arg("-f")
        .arg("image2pipe")
        .arg("-")
        .output()?;
    if output.status.success() {
        Ok(output.stdout)
    } else {
        Err(ThumbError::Ffmpeg(
            String::from_utf8_lossy(&output.stderr).to_string(),
        ))
    }
}
