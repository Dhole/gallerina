/*
use std::error::Error;
use std::path::Path;

pub enum MediaType {
    Jpeg,
    Gif,
    Png,
}

pub fn is_media(path: &Path) -> Option<MediaType> {
    let ext = match path.extension() {
        Some(e) => match e.to_str() {
            Some(e) => e.to_lowercase(),
            None => return None,
        },
        None => return None, // TODO: If no extension, read mime type
    };
    let ext = match ext.as_str() {
        "jpg" => MediaType::Jpeg,
        "jpeg" => MediaType::Jpeg,
        "jpe" => MediaType::Jpeg,
        "gif" => MediaType::Gif,
        "png" => MediaType::Png,
        _ => return None,
    };
    Some(ext)
}
*/
