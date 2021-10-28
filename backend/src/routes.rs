use http_types::mime::Mime;
use std::error::Error;
use std::fmt;
use std::path::Path;
use std::str::FromStr;
use tide::Body;
use tide::Response;

use crate::models::{queries, responses};
use crate::scanner;
use crate::state;

const HEADER_CACHE_KEY: &str = "Cache-Control";
const HEADER_CACHE_VALUE: &str = "max-age=3600";

pub type Request = tide::Request<state::State>;

pub async fn get_folder(req: Request) -> tide::Result<Body> {
    let query: queries::FolderQuery = req.query()?;
    let media = req
        .state()
        .storage
        .folder_media(&query.dir, &query.sort, query.reverse)
        .await?;
    let folders = req
        .state()
        .storage
        .folder_folders(&query.dir, &query.sort, query.reverse)
        .await?;
    Body::from_json(&responses::Folder { media, folders })
}

pub async fn get_thumb(req: Request) -> tide::Result<Response> {
    let query: queries::ThumbQuery = req.query()?;
    let mut body = Body::from_bytes(req.state().storage.thumb(&query.path)?);
    body.set_mime(Mime::from_str("image/jpeg").expect("Mime image/jpeg"));

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

pub async fn get_src(req: Request) -> tide::Result<Response> {
    let name = req.param("name")?;
    let query: queries::SrcQuery = req.query()?;
    let root = req.state().storage.root().clone();
    let mut path = root.join(Path::new(query.dir.strip_prefix('/').unwrap_or(&query.dir)));
    path.push(name);
    let path = path.canonicalize()?;
    if !path.starts_with(root) {
        return Err(http_types::Error::new(400, QueryError::PathOutOfRoot));
    }

    let mut res = Response::new(200);
    res.set_body(Body::from_file(path).await?);
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
