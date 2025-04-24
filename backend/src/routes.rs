use async_std::fs::File;
use async_std::prelude::*;
use http_types::mime::Mime;
use percent_encoding::percent_decode_str;
use std::error::Error;
use std::fmt;
use std::path::Path;
use std::str::FromStr;
use tide::{Body, Redirect, Response};
use url::Url;

use crate::magick;
use crate::models::{queries, responses};
use crate::scanner::{self, MediaType};
use crate::state::{self, Config};

const HEADER_CACHE_KEY: &str = "Cache-Control";
const HEADER_CACHE_VALUE: &str = "max-age=3600";

pub type Request = tide::Request<state::State>;

pub async fn get_folder(req: Request) -> tide::Result<Body> {
    let query: queries::FolderQuery = req.query()?;
    let media = req
        .state()
        .storage
        .folder_media(
            &query.dir,
            query.page,
            &query.sort,
            query.seed,
            query.reverse,
        )
        .await?;
    let total = media.get(0).map(|m| m.total as usize).unwrap_or(0);
    let folders = if query.page > 0 {
        vec![]
    } else {
        req.state()
            .storage
            .folder_folders(&query.dir, &query.sort, query.reverse)
            .await?
    };
    Body::from_json(&responses::Folder {
        media: media
            .into_iter()
            .map(|m| responses::MediaData { name: m.name })
            .collect(),
        folders,
        page: query.page,
        page_size: req.state().storage.page_size,
        total,
    })
}

pub async fn get_folder_recursive(req: Request) -> tide::Result<Body> {
    // Reuse FolderQuery eventhough we only care about dir
    let query: queries::FolderQuery = req.query()?;
    let media = req
        .state()
        .storage
        .folder_media_recursive(&query.dir, query.page, &query.sort, query.seed)
        .await?;
    let total = media.get(0).map(|m| m.total as usize).unwrap_or(0);
    Body::from_json(&responses::FolderRecursive {
        media: media
            .into_iter()
            .map(|m| responses::MediaDataDir {
                dir: m.dir,
                name: m.name,
            })
            .collect(),
        page: query.page,
        page_size: req.state().storage.page_size,
        total,
    })
}

pub async fn get_thumb(req: Request) -> tide::Result<Response> {
    let query: queries::ThumbQuery = req.query()?;
    let mut body = Body::from_bytes(req.state().storage.thumb(&query.path)?);
    body.set_mime(Mime::from_str("image/webp").expect("Mime image/webp"));

    let mut res = Response::new(200);
    res.set_body(body);
    res.insert_header(HEADER_CACHE_KEY, HEADER_CACHE_VALUE);
    Ok(res)
}

#[derive(Debug)]
pub enum QueryError {
    PathOutOfRoot,
}

impl fmt::Display for QueryError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "QueryError {:?}", self)
    }
}

impl Error for QueryError {}

// Helper function to get the noramlized file path from a get request used in `get_src` and
// `get_raw`
async fn helper_get_path(req: &Request) -> tide::Result<String> {
    let name = &*percent_decode_str(req.param("name")?).decode_utf8_lossy();
    let query: queries::SrcQuery = req.query()?;
    let root = req.state().storage.root().clone();
    let mut path = root.join(Path::new(query.dir.strip_prefix('/').unwrap_or(&query.dir)));
    path.push(name);
    let path = path.canonicalize().unwrap_or(path);
    if !path.starts_with(root) {
        return Err(http_types::Error::new(400, QueryError::PathOutOfRoot));
    }
    Ok(path.to_string_lossy().to_string())
}

fn redirect_url_webp(url_str: &str) -> String {
    let url = Url::parse(url_str).unwrap();
    let path = url.path();
    let query = url.query().unwrap();
    format!("{}.re.webp?{}", path, query)
}

// reencode some formats into web-friendly ones
pub async fn get_src(req: Request) -> tide::Result<Response> {
    let path = helper_get_path(&req).await?;

    let media_type = scanner::is_media(&Path::new(&path)).unwrap().unwrap();
    match media_type {
        MediaType::JXL => {
            let new_url = redirect_url_webp(req.url().as_str());
            return Ok(Redirect::new(new_url).into());
        }
        _ => {}
    };

    let (path, reencode_webp) = if let Some(stripped) = path.strip_suffix(".re.webp") {
        (stripped.to_string(), true)
    } else {
        (path, false)
    };

    let mut file = File::open(path).await?;
    let mut buf = Vec::new();
    file.read_to_end(&mut buf).await?;
    let mime = if reencode_webp {
        let Config {
            webp_quality,
            webp_compression,
            ..
        } = req.state().cfg;
        buf = magick::convert_to_webp(&buf, webp_quality, webp_compression)?;
        Mime::from_str("image/webp").unwrap()
    } else {
        Mime::sniff(&buf)?
    };

    let mut body = Body::from_bytes(buf);
    body.set_mime(mime);

    let mut res = Response::new(200);
    res.set_body(body);
    res.insert_header(HEADER_CACHE_KEY, HEADER_CACHE_VALUE);
    Ok(res)
}

// get the original file without reencoding
pub async fn get_raw(req: Request) -> tide::Result<Response> {
    let path = helper_get_path(&req).await?;
    let mut file = File::open(path).await?;
    let mut buf = Vec::new();
    file.read_to_end(&mut buf).await?;

    let mime = Mime::sniff(&buf)?;
    let mut body = Body::from_bytes(buf);
    body.set_mime(mime);

    let mut res = Response::new(200);
    res.set_body(body);
    res.insert_header(HEADER_CACHE_KEY, HEADER_CACHE_VALUE);
    Ok(res)
}

pub async fn get_status(req: Request) -> tide::Result<Body> {
    Body::from_json(&responses::Status {
        root: req.state().storage.root.to_string_lossy().to_string(),
        stats: req.state().scanner.stats().await,
        scanner_state: req.state().scanner.state().await,
    })
}

pub async fn post_scan_run(req: Request) -> tide::Result<Body> {
    let reply = req.state().scanner.request(scanner::Request::Run).await;
    Body::from_json(&responses::ScannerReply { reply })
}

pub async fn post_scan_stop(req: Request) -> tide::Result<Body> {
    let reply = req.state().scanner.request(scanner::Request::Stop).await;
    Body::from_json(&responses::ScannerReply { reply })
}
