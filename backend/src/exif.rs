use chrono::offset::{FixedOffset, Utc};
use chrono::DateTime;
use chrono::LocalResult;
use chrono::TimeZone;
use log::error;
use std::error::Error;
use std::fmt;
use std::path::Path;

#[derive(Debug, PartialEq)]
pub enum Rotation {
    D0,
    D90,
    D180,
    D270,
}

#[derive(Debug, PartialEq)]
pub struct Orientation {
    pub rotation: Rotation,
    pub mirror: bool,
}

impl Orientation {
    pub fn from_exif_value(v: u32) -> Option<Self> {
        let rotation = match v {
            1..=2 => Rotation::D0,
            3..=4 => Rotation::D180,
            5..=6 => Rotation::D90,
            7..=8 => Rotation::D270,
            _ => return None,
        };
        let mirror = match v {
            2 | 4 | 5 | 7 => true,
            _ => false,
        };
        Some(Self { rotation, mirror })
    }
}

#[derive(Debug, PartialEq)]
pub struct Exif {
    pub date_time_original: Option<i64>,
    pub orientation: Option<Orientation>,
    // make: Option<String>,
    // model: Option<String>,
    // software: Option<String>,
    // // lens: Option<String>,
    // exposure_time: Option<f64>,
    // aperture: Option<f64>,
    // iso_speed: Option<i64>,
    // focal_length: Option<f64>,
    // flash: Option<i64>,
    // exposure_time: Option<f64>,
    // exposure_program: Option<i64>,
    // gps: Option<(f64, f64)>
}

/*
pub fn ascii_to_datetime(field: &exif::Field) -> Result<Option<DateTime<Utc>>, Box<dyn Error>> {
    match field.value {
        exif::Value::Ascii(ref vec) => {
            let exif_datetime = exif::DateTime::from_ascii(vec[0].as_ref())?;
            Ok(Some(exif_to_datetime(&exif_datetime)?))
        }
        _ => Ok(None),
    }
}
*/

#[derive(Debug)]
pub struct ExifDateTimeError;

impl fmt::Display for ExifDateTimeError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Exif DateTime Error")
    }
}

impl Error for ExifDateTimeError {}

pub fn exif_to_datetime(dt: &exif::DateTime) -> Result<DateTime<Utc>, Box<dyn Error>> {
    // Option B
    let tz = FixedOffset::east_opt(dt.offset.unwrap_or(0) as i32 * 60).ok_or(ExifDateTimeError)?;
    Ok(
        match Utc.ymd_opt(dt.year as i32, dt.month as u32, dt.day as u32) {
            LocalResult::Single(d) => d
                .and_hms_nano_opt(
                    dt.hour as u32,
                    dt.minute as u32,
                    dt.second as u32,
                    dt.nanosecond.unwrap_or(0),
                )
                .ok_or(ExifDateTimeError)?,
            _ => return Err(Box::new(ExifDateTimeError)),
        } - tz,
    )
}

pub fn exif_field_to_datetime(field: &exif::Field) -> Result<DateTime<Utc>, Box<dyn Error>> {
    match field.value {
        exif::Value::Ascii(ref vec) => {
            let value: &[u8] = &vec[0].as_ref();
            // FIX: I have encountered Date/Time like this: `2013:07:04 21:02:5`
            const LEN: usize = 19;
            let mut buf = [b'0'; LEN];
            let len = std::cmp::min(buf.len(), value.len());
            buf[..len].copy_from_slice(&value[..len]);
            let exif_datetime = exif::DateTime::from_ascii(&buf)?;
            Ok(exif_to_datetime(&exif_datetime)?)
        }
        _ => unimplemented!(),
    }
}

impl Exif {
    pub fn new(path: &Path) -> Result<Self, Box<dyn Error>> {
        let file = std::fs::File::open(path)?;
        let mut bufreader = std::io::BufReader::new(&file);
        let exifreader = exif::Reader::new();
        let exif = exifreader.read_from_container(&mut bufreader)?;
        let date_time_original =
            match exif.get_field(exif::Tag::DateTimeOriginal, exif::In::PRIMARY) {
                Some(field) => match exif_field_to_datetime(field) {
                    Ok(dt) => Some(dt.timestamp()),
                    Err(e) => {
                        error!("Parsing exif DateTimeOriginal from {:?}: {:?}", path, e);
                        None
                    }
                },
                None => None,
            };
        let orientation = match exif.get_field(exif::Tag::Orientation, exif::In::PRIMARY) {
            Some(field) => match field.value.get_uint(0) {
                Some(v) => match Orientation::from_exif_value(v) {
                    Some(ori) => Some(ori),
                    None => {
                        error!("Invalid exif orientation value from {:?}: {}", path, v);
                        None
                    }
                },
                None => {
                    error!(
                        "Invalid exif orientation value type from {:?}: {:?}",
                        path, field
                    );
                    None
                }
            },
            None => None,
        };
        Ok(Self {
            date_time_original,
            orientation,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_exif_to_datetime() {
        let exif_dt = exif::DateTime {
            year: 2021,
            month: 09,
            day: 19,
            hour: 14,
            minute: 05,
            second: 40,
            nanosecond: None,
            offset: Some(2 * 60),
        };
        let dt = exif_to_datetime(&exif_dt).expect("exif to datetime");
        assert_eq!(format!("{:?}", dt), "2021-09-19T12:05:40Z");
    }

    #[test]
    fn test_exif_new() {
        let exif =
            Exif::new(Path::new("/home/dev/git/exif-samples/jpg/Canon_40D.jpg")).expect("exif new");
        assert_eq!(
            exif,
            Exif {
                date_time_original: Some(1212162961),
                orientation: None,
            }
        );
    }
}
