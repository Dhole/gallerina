// use async_lock::Barrier;
use async_recursion::async_recursion;
use async_std::channel::{self, Receiver, Sender};
use async_std::task::{self, JoinHandle};
use awaitgroup::{WaitGroup, Worker};
use chrono::prelude::*;
use futures::executor::ThreadPoolBuilder;
use log::{debug, error};
use parallel_stream::{from_stream, prelude::*};
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::error::Error;
use std::fmt;
use std::mem;
// use std::fs;
use async_std::sync::{Arc, Mutex, RwLock};
use std::io;
use std::path::Path;
use std::path::PathBuf;
use std::time;
use std::time::Duration;

use crate::exif::Exif;
use crate::ffmpeg;
use crate::magick;
use crate::models::tables;
use crate::models::views;
use crate::state::Storage;
// use crate::utils::MediaType::*;

pub const THUMB_SIZE: u16 = 512;
pub const THUMB_QUALITY: u8 = 80;
pub const MAX_SQL_TX_SIZE: usize = 1024;

#[derive(Debug)]
pub enum ScanError {
    Io(PathBuf, io::Error),
    Sqlx(sqlx::Error),
    Heed(heed::Error),
}

impl From<heed::Error> for ScanError {
    fn from(error: heed::Error) -> Self {
        Self::Heed(error)
    }
}

impl From<sqlx::Error> for ScanError {
    fn from(error: sqlx::Error) -> Self {
        Self::Sqlx(error)
    }
}

impl fmt::Display for ScanError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

impl Error for ScanError {}

impl From<(PathBuf, io::Error)> for ScanError {
    fn from(path_error: (PathBuf, io::Error)) -> Self {
        Self::Io(path_error.0, path_error.1)
    }
}

#[derive(Debug, Clone, Serialize)]
pub enum State {
    Idle,
    Scanning,
    Indexing,
    Error(String),
}

enum ScanState {
    Idle,
    Scanning(JoinHandle<()>),
    Indexing(JoinHandle<()>, IndexerHandle),
    Error(String),
}

impl Into<State> for &ScanState {
    fn into(self) -> State {
        match self {
            ScanState::Idle => State::Idle,
            ScanState::Scanning(_) => State::Scanning,
            ScanState::Indexing(..) => State::Indexing,
            ScanState::Error(e) => State::Error(e.to_string()),
        }
    }
}

#[derive(Debug, Serialize, Clone, Copy)]
pub struct Stats {
    last_scan_start: Option<chrono::DateTime<Local>>,
    last_scan_end: Option<chrono::DateTime<Local>>,
    scan_folders_count: usize,
    scan_files_count: usize,
    scan_folders_total: usize,
    scan_files_total: usize,
}

impl Stats {
    pub fn new() -> Self {
        Self {
            last_scan_start: None,
            last_scan_end: None,
            scan_folders_count: 0,
            scan_files_count: 0,
            scan_folders_total: 0,
            scan_files_total: 0,
        }
    }

    pub fn reset_counters(&mut self) {
        self.scan_folders_count = 0;
        self.scan_files_count = 0;
        self.scan_folders_total = 0;
        self.scan_files_total = 0;
    }
}

#[derive(Clone)]
pub struct Scanner {
    state: Arc<RwLock<ScanState>>,
    storage: Storage,
    n_threads: usize,
    stats: Arc<RwLock<Stats>>,
}

#[derive(Debug)]
pub enum Request {
    Run,
    Stop,
}

#[derive(Debug, Serialize)]
pub enum Reply {
    Idle,
    NotIdle,
    OK,
}

impl Scanner {
    pub fn new(storage: Storage, n_threads: usize) -> Self {
        Self {
            state: Arc::new(RwLock::new(ScanState::Idle)),
            storage,
            n_threads,
            stats: Arc::new(RwLock::new(Stats::new())),
        }
    }

    async fn task_fn_scan_dir(self) {
        let scan_dir = match scan_dir(self.stats.clone(), self.storage.root.clone()).await {
            Ok(scan_dir) => scan_dir,
            Err(err) => {
                error!("scan_dir: {:?}", err);
                // State transition to Error
                debug!("-> ScanState::Error");
                *self.state.write().await = ScanState::Error(format!("scan_dir: {:?}", err));
                return;
            }
        };
        let (indexer, indexer_handle) =
            Indexer::start(self.storage.clone(), self.stats.clone(), self.n_threads);
        // State transition to Indexing
        debug!("-> ScanState::Indexing");
        *self.state.write().await = ScanState::Indexing(
            task::spawn(
                self.clone()
                    .task_fn_index(indexer, indexer_handle.clone(), scan_dir),
            ),
            indexer_handle,
        );
    }

    async fn task_fn_index(
        self,
        indexer: Indexer,
        mut indexer_handle: IndexerHandle,
        scan_dir: ScanDir,
    ) {
        match indexer.update(&scan_dir).await {
            Ok(_) => {}
            Err(err) => {
                error!("indexer update: {:?}", err);
                // State transition to Error
                debug!("-> ScanState::Error");
                *self.state.write().await = ScanState::Error(format!("indexer update: {:?}", err));
                return;
            }
        }
        indexer_handle.wait_stop().await;
        self.stats.write().await.last_scan_end = Some(Local::now());
        // State transition to Idle
        debug!("-> ScanState::Idle");
        *self.state.write().await = ScanState::Idle;
    }

    pub async fn request(&self, r: Request) -> Reply {
        match r {
            Request::Run => {
                let mut state = self.state.write().await;
                if let ScanState::Idle | ScanState::Error(_) = &*state {
                    let mut stats = self.stats.write().await;
                    stats.last_scan_start = Some(Local::now());
                    stats.reset_counters();
                    drop(stats);
                    // State transition to Scanning
                    debug!("-> ScanState::Scanning");
                    *state = ScanState::Scanning(task::spawn(self.clone().task_fn_scan_dir()));
                    Reply::OK
                } else {
                    Reply::NotIdle
                }
            }
            Request::Stop => {
                debug!("Request::Stop");
                let mut state = self.state.write().await;
                let old_state = mem::replace(&mut *state, ScanState::Idle);
                match old_state {
                    ScanState::Idle | ScanState::Error(_) => Reply::Idle,
                    ScanState::Scanning(handle) => {
                        debug!("scan_dir handle.cancel");
                        handle.cancel().await;
                        debug!("-> ScanState::Idle");
                        *state = ScanState::Idle;
                        Reply::OK
                    }
                    ScanState::Indexing(update_handle, mut indexer_handle) => {
                        debug!("indexer update handle.cancel");
                        update_handle.cancel().await;
                        debug!("indexer handle.stop");
                        indexer_handle.stop().await;
                        debug!("-> ScanState::Idle");
                        *state = ScanState::Idle;
                        Reply::OK
                    }
                }
            }
        }
    }

    pub async fn state(&self) -> State {
        (&*self.state.read().await).into()
    }

    pub async fn stats(&self) -> Stats {
        *self.stats.read().await
    }
}

pub enum MediaType {
    Jpeg,
    Gif,
    Png,
    Mp4,
}

use MediaType::*;

#[derive(Debug)]
pub enum IsMediaError {
    PathEncoding,
}

pub fn is_media(path: &Path) -> Result<Option<MediaType>, IsMediaError> {
    let ext = match path.extension() {
        Some(e) => match e.to_str() {
            Some(e) => e.to_lowercase(),
            None => return Err(IsMediaError::PathEncoding),
        },
        None => return Ok(None), // TODO: If no extension, read mime type
    };
    let ext = match ext.as_str() {
        "jpg" => Jpeg,
        "jpeg" => Jpeg,
        "jpe" => Jpeg,
        "gif" => Gif,
        "png" => Png,
        "mp4" => Mp4,
        _ => return Ok(None),
    };
    Ok(Some(ext))
}

#[derive(Debug)]
pub enum ThumbError {
    IsMediaError(IsMediaError),
    Io(io::Error),
    Magick(magick_rust::MagickError),
    Ffmpeg(String),
}

impl From<io::Error> for ThumbError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<IsMediaError> for ThumbError {
    fn from(error: IsMediaError) -> Self {
        Self::IsMediaError(error)
    }
}

impl fmt::Display for ThumbError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

impl Error for ThumbError {}

fn make_thumb<P>(filepath: P, _media_exif: &Option<Exif>) -> Result<Vec<u8>, ThumbError>
where
    P: AsRef<Path>,
{
    let filepath = filepath.as_ref();
    let ext = filepath
        .extension()
        .unwrap_or_default()
        .to_string_lossy()
        .to_lowercase();
    let thumb = if ext == "mp4" {
        ffmpeg::make_thumb(&*filepath.to_string_lossy())
    } else {
        magick::make_thumb(&*filepath.to_string_lossy()).map_err(|err| ThumbError::Magick(err))
    }?;
    return Ok(thumb);
}

struct CompareResult<'a> {
    new: Vec<&'a str>,
    update: Vec<&'a str>,
    del: Vec<&'a str>,
}

#[derive(Debug)]
struct MediaThumb {
    media: tables::Image,
    thumb: Option<Vec<u8>>,
}

impl MediaThumb {
    fn new<'a>(dir: &Path, name: &'a str, mtime: i64) -> Self {
        Self {
            media: tables::Image {
                path: subpath(dir, name).to_string_lossy().to_string(),
                name: name.to_string(),
                timestamp: 0,
                mtime: mtime,
                dir: dir.to_string_lossy().to_string(),
            },
            thumb: None,
        }
    }
}

struct IndexRequest {
    new: Vec<MediaThumb>,
    update: Vec<MediaThumb>,
}

fn compare_entries<'a>(
    scan: &HashMap<&'a str, i64>,
    db: &HashMap<&'a str, i64>,
) -> CompareResult<'a> {
    let scan_set: HashSet<&str> = scan.keys().map(|s| *s).collect();
    let db_set: HashSet<&str> = db.keys().map(|s| *s).collect();

    // new = scan - db
    let new: Vec<&str> = scan_set.difference(&db_set).map(|s| *s).collect();
    // update = scan & db WHERE scan.mtime != db.mtime
    let update: Vec<&str> = scan_set
        .intersection(&db_set)
        .filter(|s| scan.get(*s).expect("key found") != db.get(*s).expect("key found"))
        .map(|s| *s)
        .collect();
    // del = db - scan
    let del: Vec<&str> = db_set.difference(&scan_set).map(|s| *s).collect();

    CompareResult { new, update, del }
}

fn fullpath<P1, P2: AsRef<Path>>(root: P1, path: P2) -> PathBuf
where
    PathBuf: From<P1>,
{
    let mut full = PathBuf::from(root);
    let path = path.as_ref();
    full.push(path.strip_prefix("/").unwrap_or(path));
    full
}

fn subpath<P1, P2: AsRef<Path>>(dir: P1, sub: P2) -> PathBuf
where
    PathBuf: From<P1>,
{
    let mut path = PathBuf::from(dir);
    if &sub.as_ref() != &Path::new(".") {
        path.push(sub);
    }
    path
}

#[derive(Clone)]
struct IndexerHandle {
    stop: Arc<RwLock<bool>>,
    thumbs_req: (Sender<IndexRequest>, Receiver<IndexRequest>),
    thumbs_res: (Sender<IndexRequest>, Receiver<IndexRequest>),
    // pool_gen_thumbs: ThreadPool,
    wg_gen_thumbs: Arc<Mutex<WaitGroup>>,
    task_media_insert: Arc<Mutex<Option<task::JoinHandle<()>>>>,
    wg_media_insert: Arc<Mutex<WaitGroup>>,
}

impl IndexerHandle {
    pub async fn stop(&mut self) {
        debug!("stop -> true");
        *self.stop.write().await = true;
        debug!("closing thumbs_req channel");
        self.thumbs_req.0.close();
        self.thumbs_req.1.close();
        debug!("closing thumbs_res channel");
        self.thumbs_res.0.close();
        self.thumbs_res.1.close();

        debug!("waiting on wg_gen_thumbs");
        self.wg_gen_thumbs.lock().await.wait().await;
        debug!("waiting on wg_media_insert");
        self.wg_media_insert.lock().await.wait().await;

        let task_media_insert = self.task_media_insert.lock().await.take();
        match task_media_insert {
            Some(handle) => {
                debug!("media_insert handle.cancel");
                handle.cancel().await;
            }
            None => {}
        }
        // drop(self.pool_gen_thumbs);
    }

    pub async fn wait_stop(&mut self) {
        debug!("closing thumbs_req channel");
        self.thumbs_req.0.close();
        self.thumbs_req.1.close();

        debug!("waiting on wg_gen_thumbs");
        self.wg_gen_thumbs.lock().await.wait().await;

        debug!("closing thumbs_res channel");
        self.thumbs_res.0.close();
        self.thumbs_res.1.close();

        debug!("waiting on wg_media_insert");
        self.wg_media_insert.lock().await.wait().await;

        let task_media_insert = self.task_media_insert.lock().await.take();
        match task_media_insert {
            Some(handle) => {
                debug!("media_insert handle.cancel");
                handle.cancel().await;
            }
            None => {}
        }
        // drop(self.pool_gen_thumbs);
    }
}

#[derive(Clone)]
struct Indexer {
    state: Storage,
    stats: Arc<RwLock<Stats>>,
    thumbs_req: (Sender<IndexRequest>, Receiver<IndexRequest>),
    // thumbs_res: (Sender<IndexRequest>, Receiver<IndexRequest>),
}

type Tx = sqlx::Transaction<'static, sqlx::Sqlite>;

#[derive(Debug)]
struct Batch<'a> {
    max: usize,
    count: usize,
    db: &'a sqlx::SqlitePool,
    tx: Tx,
}

impl<'a> Batch<'a> {
    pub async fn new(max: usize, db: &'a sqlx::SqlitePool) -> Result<Batch<'a>, sqlx::Error> {
        let tx = db.begin().await?;
        Ok(Self {
            max,
            count: 0,
            db: db,
            tx,
        })
    }

    async fn inc_maybe_commit_begin(&mut self) -> Result<(), sqlx::Error> {
        self.count += 1;
        if self.count >= self.max {
            let tx = std::mem::replace(&mut self.tx, self.db.begin().await?);
            tx.commit().await?;
            self.count = 0;
        }
        Ok(())
    }

    pub async fn commit(self) -> Result<(), sqlx::Error> {
        self.tx.commit().await?;
        Ok(())
    }
}

// use core::pin::Pin;
use either::Either;
use futures_core::future::BoxFuture;
use futures_core::stream::BoxStream;
// use futures_core::Stream;
use async_stream::stream;
use sqlx::sqlite::{SqliteQueryResult, SqliteRow, SqliteStatement, SqliteTypeInfo};
use sqlx::Describe;
use sqlx::Execute;

// use std::alloc::Global;

impl<'c> sqlx::Executor<'c> for &'c mut Batch<'_> {
    type Database = sqlx::Sqlite;
    fn fetch_many<'e, 'q: 'e, E: 'q>(
        self,
        query: E,
    ) -> BoxStream<'e, Result<Either<SqliteQueryResult, SqliteRow>, sqlx::Error>>
    where
        'c: 'e,
        E: Execute<'q, Self::Database>,
    {
        Box::pin(stream! {
                self.inc_maybe_commit_begin().await?;
                for await value in self.tx.fetch_many(query) {
                    yield value;
                }
        })
    }

    fn fetch_optional<'e, 'q: 'e, E: 'q>(
        self,
        query: E,
    ) -> BoxFuture<'e, Result<Option<SqliteRow>, sqlx::Error>>
    where
        'c: 'e,
        E: Execute<'q, Self::Database>,
    {
        Box::pin(async move {
            self.inc_maybe_commit_begin().await?;
            self.tx.fetch_optional(query).await
        })
    }

    fn prepare_with<'e, 'q: 'e>(
        self,
        sql: &'q str,
        _parameters: &'e [SqliteTypeInfo],
    ) -> BoxFuture<'e, Result<SqliteStatement<'q>, sqlx::Error>>
    where
        'c: 'e,
    {
        self.tx.prepare_with(sql, _parameters)
    }

    #[doc(hidden)]
    fn describe<'e, 'q: 'e>(
        self,
        sql: &'q str,
    ) -> BoxFuture<'e, Result<Describe<sqlx::Sqlite>, sqlx::Error>>
    where
        'c: 'e,
    {
        self.tx.describe(sql)
    }
}

impl Indexer {
    pub fn start(
        state: Storage,
        stats: Arc<RwLock<Stats>>,
        n_threads: usize,
    ) -> (Self, IndexerHandle) {
        let thumbs_req = channel::bounded(n_threads);
        let thumbs_res = channel::bounded(n_threads);
        let pool_gen_thumbs = ThreadPoolBuilder::new()
            .pool_size(n_threads)
            .name_prefix("thumb-")
            .create()
            .expect("ThreadPool create");
        let stop = Arc::new(RwLock::new(false));
        let wg_gen_thumbs = WaitGroup::new();
        for i in 0..n_threads {
            pool_gen_thumbs.spawn_ok(Self::gen_thumbs(
                i,
                stop.clone(),
                wg_gen_thumbs.worker(),
                state.clone(),
                thumbs_req.1.clone(),
                thumbs_res.0.clone(),
            ));
        }
        let wg_media_insert = WaitGroup::new();
        let task_media_insert = Arc::new(Mutex::new(Some(task::spawn(Self::media_insert(
            stop.clone(),
            wg_media_insert.worker(),
            state.clone(),
            stats.clone(),
            thumbs_res.1.clone(),
        )))));

        (
            Self {
                state,
                stats,
                thumbs_req: thumbs_req.clone(),
                // thumbs_res: thumbs_res.clone(),
            },
            IndexerHandle {
                stop,
                thumbs_req,
                thumbs_res,
                // pool_gen_thumbs,
                wg_gen_thumbs: Arc::new(Mutex::new(wg_gen_thumbs)),
                task_media_insert,
                wg_media_insert: Arc::new(Mutex::new(wg_media_insert)),
            },
        )
    }

    async fn gen_thumbs(
        id: usize,
        stop: Arc<RwLock<bool>>,
        wg: Worker,
        state: Storage,
        thumbs_req_receiver: Receiver<IndexRequest>,
        thumbs_res_sender: Sender<IndexRequest>,
    ) {
        debug!("gen_thumbs {} started", id);
        while let Ok(mut res) = thumbs_req_receiver.recv().await {
            for entry in res.new.iter_mut().chain(res.update.iter_mut()) {
                if *stop.read().await {
                    break;
                }
                let path = fullpath(&state.root, &entry.media.path);
                let media_exif = Exif::new(&path).ok();
                entry.media.timestamp = media_exif
                    .as_ref()
                    .map(|e| e.date_time_original)
                    .unwrap_or(None) // This can happen when a file doesn't have EXIF data.
                    .unwrap_or(entry.media.mtime);
                entry.thumb = match make_thumb(&path, &media_exif) {
                    Ok(thumb) => Some(thumb),
                    Err(err) => {
                        error!(
                            "cannot make thumb for {:?}: {:?}",
                            fullpath(&state.root, &entry.media.path),
                            err
                        );
                        None
                    }
                };
            }
            match thumbs_res_sender.send(res).await {
                Ok(_) => {}
                Err(_) => break, // thumbs_res channel closed, we should terminate.
            };
        }
        debug!("gen_thumbs {} done", id);
        wg.done();
    }

    async fn media_insert(
        stop: Arc<RwLock<bool>>,
        wg: Worker,
        mut state: Storage,
        mut stats: Arc<RwLock<Stats>>,
        thumbs_res_receiver: Receiver<IndexRequest>,
    ) {
        while let Ok(res) = thumbs_res_receiver.recv().await {
            if *stop.read().await {
                break;
            }
            while let Err(err) = Self::media_insert_try(&stop, &mut state, &mut stats, &res).await {
                error!("media_insert: {:?}.  Retrying in 10 seconds", err);
                task::sleep(Duration::from_secs(10)).await;
            }
        }
        debug!("media_insert done");
        wg.done();
    }

    async fn media_insert_try(
        stop: &Arc<RwLock<bool>>,
        state: &mut Storage,
        stats: &mut Arc<RwLock<Stats>>,
        res: &IndexRequest,
    ) -> Result<(), ScanError> {
        {
            let mut wtxn = state.thumb_db_env.write_txn()?;
            for entry in res.new.iter().chain(res.update.iter()) {
                if let Some(thumb) = &entry.thumb {
                    state.thumb_db.put(&mut wtxn, &entry.media.path, thumb)?;
                }
            }
            wtxn.commit()?;
        }
        // let mut tx = state.db.begin().await?;
        let mut batch = Batch::new(MAX_SQL_TX_SIZE, &state.db).await?;
        for entry in &res.new {
            if *stop.read().await {
                return Ok(());
            }
            let media = &entry.media;
            sqlx::query(
                "INSERT INTO image (path, name, dir, mtime, timestamp) VALUES (?, ?, ?, ?, ?)",
            )
            .bind(&media.path)
            .bind(&media.name)
            .bind(&media.dir)
            .bind(media.mtime)
            .bind(media.timestamp)
            .execute(&mut batch)
            .await?;
        }
        for entry in &res.update {
            if *stop.read().await {
                return Ok(());
            }
            let media = &entry.media;
            sqlx::query("UPDATE image SET mtime = ?, timestamp = ? WHERE path = ?")
                .bind(media.mtime)
                .bind(media.timestamp)
                .bind(&media.path)
                .execute(&mut batch)
                .await?;
        }
        batch.commit().await?;
        stats.write().await.scan_files_count += res.new.len() + res.update.len();
        Ok(())
    }

    pub async fn update(&self, scan_dir: &ScanDir) -> Result<(), ScanError> {
        let parent = &Path::new("/");
        self.stats.write().await.scan_folders_count += 1;
        self.update_inner(parent, scan_dir).await
    }

    #[async_recursion]
    async fn update_inner(self: &Self, parent: &Path, scan_dir: &ScanDir) -> Result<(), ScanError> {
        let path = subpath(parent, &scan_dir.name);
        let path_string = &*path.to_string_lossy();
        // println!("path_string = {:?}", path_string);

        // Query dir = path in SQL folder -> db_subdirs
        let db_subdirs: Vec<views::FolderScan> =
            sqlx::query_as("SELECT name, mtime FROM folder WHERE dir = ?")
                .bind(path_string)
                .fetch_all(&self.state.db)
                .await?;

        let scan_subdirs: HashMap<&str, i64> = scan_dir
            .dirs
            .iter()
            .map(|v| (v.name.as_str(), v.mtime))
            .collect();
        let db_subdirs: HashMap<&str, i64> = db_subdirs
            .iter()
            .map(|v| (v.name.as_str(), v.mtime))
            .collect();
        let subdirs_cmp = compare_entries(&scan_subdirs, &db_subdirs);

        // new_dirs -> create SQL in folder
        let mut batch = Batch::new(MAX_SQL_TX_SIZE, &self.state.db).await?;
        for name in &subdirs_cmp.new {
            let mtime = *(scan_subdirs.get(name).expect("key found"));
            sqlx::query("INSERT INTO folder (path, name, dir, mtime) VALUES (?, ?, ?, ?)")
                .bind(&*subpath(&path, name).to_string_lossy())
                .bind(name)
                .bind(path_string)
                .bind(mtime)
                .execute(&mut batch)
                .await?;
        }
        // update_dirs -> update SQL in folder
        for name in &subdirs_cmp.update {
            let mtime = *(scan_subdirs.get(name).expect("key found"));
            sqlx::query("UPDATE folder SET mtime = ? WHERE path = ?")
                .bind(mtime)
                .bind(&*subpath(&path, name).to_string_lossy())
                .execute(&mut batch)
                .await?;
        }
        // del_dirs -> del SQL in folder + del dir in path SQL in folder + del dir in path SQL in
        // folder & thumb
        for name in &subdirs_cmp.del {
            let del_path = subpath(&path, name);
            let del_path_str = &*del_path.to_string_lossy();
            sqlx::query("DELETE FROM image WHERE dir LIKE ?")
                .bind(format!("{}%", del_path_str))
                .execute(&mut batch)
                .await?;
            sqlx::query("DELETE FROM folder WHERE dir LIKE ?")
                .bind(format!("{}%", del_path_str))
                .execute(&mut batch)
                .await?;
            sqlx::query("DELETE FROM folder WHERE path = ?")
                .bind(del_path_str)
                .execute(&mut batch)
                .await?;
        }
        batch.commit().await?;

        {
            let mut wtxn = self.state.thumb_db_env.write_txn()?;
            {
                for name in &subdirs_cmp.del {
                    let mut iter = self
                        .state
                        .thumb_db
                        .prefix_iter_mut(&mut wtxn, &*subpath(&path, name).to_string_lossy())?;
                    while let Some(_) = iter.next() {
                        iter.del_current()?;
                    }
                }
            }
            wtxn.commit()?;
        }

        self.stats.write().await.scan_folders_count += scan_dir.dirs.len();

        // Query dir = path in SQL images -> db_files
        let db_files: Vec<views::MediaScan> =
            sqlx::query_as("SELECT name, mtime FROM image WHERE dir = ?")
                .bind(path.to_string_lossy().to_string())
                .fetch_all(&self.state.db)
                .await?;

        let scan_files: HashMap<&str, i64> = scan_dir
            .files
            .iter()
            .map(|v| (v.name.as_str(), v.mtime))
            .collect();
        let db_files: HashMap<&str, i64> = db_files
            .iter()
            .map(|v| (v.name.as_str(), v.mtime))
            .collect();
        let files_cmp = compare_entries(&scan_files, &db_files);
        // Substract unchanged + deleted files
        self.stats.write().await.scan_files_count +=
            scan_dir.files.len() - files_cmp.new.len() - files_cmp.update.len();

        // new_files = scan_dir.files - db_files
        // update_files = scan_dir.files & db_files WHERE scandir_file.mtime != db_file.mtime
        // del_files = db_files - scan_dir.files

        // del_files -> del thumb + del SQL in image
        let mut batch = Batch::new(MAX_SQL_TX_SIZE, &self.state.db).await?;
        for name in &files_cmp.del {
            sqlx::query("DELETE FROM image WHERE path = ?")
                .bind(&*subpath(&path, name).to_string_lossy())
                .execute(&mut batch)
                .await?;
        }
        batch.commit().await?;

        {
            let mut wtxn = self.state.thumb_db_env.write_txn()?;
            for name in &files_cmp.del {
                self.state
                    .thumb_db
                    .delete(&mut wtxn, &*subpath(&path, name).to_string_lossy())?;
            }
            wtxn.commit()?;
        }

        // new_files -> gen thumb + set thumb + create SQL in image
        // update_files -> gen thumb + set thumb + update SQL in image
        let index_req = IndexRequest {
            new: files_cmp
                .new
                .iter()
                .map(|name| {
                    MediaThumb::new(&path, name, *(scan_files.get(name).expect("key found")))
                })
                .collect(),
            update: files_cmp
                .update
                .iter()
                .map(|name| {
                    MediaThumb::new(&path, name, *(scan_files.get(name).expect("key found")))
                })
                .collect(),
        };
        match self.thumbs_req.0.send(index_req).await {
            Ok(_) => {}
            Err(_) => return Ok(()), // thumbs_req channel closed, we should terminate.
        }

        // Recurse for each scan_dir.dirs
        for subdir in &scan_dir.dirs {
            self.update_inner(&path, subdir).await?;
        }
        Ok(())
    }
}

#[derive(Debug)]
struct ScanFile {
    name: String,
    mtime: i64,
}

#[derive(Debug)]
struct ScanDir {
    name: String,
    mtime: i64,
    dirs: Vec<ScanDir>,
    files: Vec<ScanFile>,
}

async fn scan_dir(mut stats: Arc<RwLock<Stats>>, dir: PathBuf) -> Result<ScanDir, ScanError> {
    let mut stats_lock = stats.write().await;
    stats_lock.scan_folders_total = 0;
    stats_lock.scan_files_total = 0;
    drop(stats_lock);
    let (_, scan_dir) = scan_dir_inner(&mut stats, &dir, ".".to_string(), 0).await?;
    Ok(scan_dir)
}

enum Entry {
    Dir(PathBuf, String, i64),
    File(String, i64),
}

// The returned bool is true when this dir has media.
// TODO: Find a more efficient parallelization strategy
#[async_recursion]
async fn scan_dir_inner(
    mut stats: &mut Arc<RwLock<Stats>>,
    dir: &Path,
    name: String,
    mtime: i64,
) -> Result<(bool, ScanDir), ScanError> {
    let entries: Vec<Option<Entry>> = from_stream(
        async_std::fs::read_dir(dir)
            .await
            .map_err(|e| (dir.to_path_buf(), e))?,
    )
    .into_par_stream()
    .map(|entry_res| async {
        let entry = match entry_res {
            Ok(entry) => entry,
            Err(err) => {
                error!("Reading DirEntry: {:?}", err);
                return None;
            }
        };
        let entry_name = entry.file_name().to_string_lossy().to_string();
        if entry_name.starts_with(".") {
            // Skip hidden entries
            return None;
        }
        let metadata = match entry.metadata().await {
            Ok(v) => v,
            Err(e) => {
                error!("Reading entry metadata {:?}: {:?}", entry.path(), e);
                return None;
            }
        };
        let modified = match metadata.modified() {
            Ok(v) => v,
            Err(e) => {
                error!("Reading metadata modified {:?}: {:?}", entry.path(), e);
                return None;
            }
        };
        let path = entry.path();
        let entry_mtime = modified
            .duration_since(time::UNIX_EPOCH)
            .expect("modified.duration_since")
            .as_secs() as i64;
        if metadata.is_dir() {
            Some(Entry::Dir(path.into(), entry_name, entry_mtime))
        } else if metadata.is_file() && is_media(&path.as_ref()).unwrap_or(None).is_some() {
            Some(Entry::File(entry_name, entry_mtime))
        } else {
            None
        }
    })
    .collect()
    .await;

    let mut subdirs: Vec<(PathBuf, String, i64)> = Vec::new();
    let mut files: Vec<ScanFile> = Vec::new();
    entries.into_iter().for_each(|opt| {
        if let Some(entry) = opt {
            match entry {
                Entry::Dir(path, name, mtime) => subdirs.push((path, name, mtime)),
                Entry::File(name, mtime) => files.push(ScanFile { name, mtime }),
            }
        }
    });
    stats.write().await.scan_files_total += files.len();
    let mut dirs = Vec::with_capacity(subdirs.len());
    for (subdir, name, mtime) in subdirs {
        let (dir_has_media, dir) = scan_dir_inner(&mut stats, &subdir, name, mtime).await?;
        if dir_has_media {
            dirs.push(dir);
        }
    }
    files.shrink_to_fit();
    dirs.shrink_to_fit();
    let has_media = files.len() > 0 || dirs.len() > 0;

    if has_media {
        stats.write().await.scan_folders_total += 1;
    }
    Ok((
        has_media,
        ScanDir {
            name,
            mtime,
            dirs,
            files,
        },
    ))
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::models::tables::{Folder, Image};
    use crate::state::{StateConfig, Storage};
    use sha3::{Digest, Sha3_256};
    use tempfile::TempDir;

    /*
    #[async_std::test]
    async fn test_scan_dir() {
        let stats = Arc::new(RwLock::new(Stats::new()));
        let d = scan_dir(stats, PathBuf::from("/data/media/Images/scripts"))
            .await
            .expect("scan_dir");
        println!("{:#?}", d);
    }
    */

    fn hash(input: &[u8]) -> Vec<u8> {
        let mut hasher = Sha3_256::new();
        hasher.update(input);
        hasher.finalize()[..].to_vec()
    }

    struct ScanResult {
        thumb_kvs: HashMap<String, Vec<u8>>,
        thumb_keys: Vec<String>,
        folders: Vec<Folder>,
        media: Vec<Image>,
    }

    async fn do_scan(cfg: StateConfig<'_>) -> ScanResult {
        let state = Storage::new(&cfg).await.expect("Storage::new");
        let stats = Arc::new(RwLock::new(Stats::new()));
        let scan_dir = scan_dir(stats.clone(), cfg.root.clone())
            .await
            .expect("scan_dir");
        // println!("scan_dir: {:?}", scan_dir);
        let (indexer, mut indexer_handle) = Indexer::start(state.clone(), stats.clone(), 4);
        indexer.update(&scan_dir).await.expect("indexer.update");
        indexer_handle.wait_stop().await;
        /*
        let mut scanner = Scanner::new(state.clone(), 4);
        scanner.request(Request::Run).await;
        loop {
            let state = scanner.state().await;
            match state {
                State::Idle => break,
                State::Error(err) => {
                    panic!("{}", err)
                }
                State::Scanning | State::Indexing => {
                    println!("{:?}", state);
                }
            }
            task::sleep(Duration::from_secs(1)).await;
        }
        */

        let tx = state.thumb_db_env.read_txn().unwrap();
        let mut kvs: HashMap<String, Vec<u8>> = HashMap::new();
        let mut keys: Vec<String> = Vec::new();
        let mut iter = state.thumb_db.iter(&tx).unwrap();
        while let Some(kv) = iter.next() {
            let kv = kv.unwrap();
            kvs.insert(kv.0.to_string(), hash(kv.1));
            keys.push(kv.0.to_string());
        }
        keys.sort();

        let folders: Vec<Folder> =
            sqlx::query_as("SELECT path, name, dir, mtime FROM folder ORDER BY path")
                .fetch_all(&state.db)
                .await
                .unwrap();

        let media: Vec<Image> =
            sqlx::query_as("SELECT path, name, dir, mtime, timestamp FROM image ORDER BY path")
                .fetch_all(&state.db)
                .await
                .unwrap();

        ScanResult {
            thumb_kvs: kvs,
            thumb_keys: keys,
            folders,
            media,
        }
    }

    macro_rules! folder {
        ($path:expr, $name:expr, $mtime:expr, $dir:expr) => {
            Folder {
                path: $path.to_string(),
                name: $name.to_string(),
                mtime: $mtime as i64,
                dir: $dir.map(|s: &str| s.to_string()),
            }
        };
    }

    macro_rules! image {
        ($path:expr, $name:expr, $dir:expr, $mtime:expr, $ts:expr) => {
            Image {
                path: $path.to_string(),
                name: $name.to_string(),
                dir: $dir.to_string(),
                mtime: $mtime as i64,
                timestamp: $ts as i64,
            }
        };
    }

    #[async_std::test]
    async fn test_update() {
        // tide::log::with_level(tide::log::LevelFilter::Debug);
        env_logger::init();

        let temp_dir = TempDir::new().expect("new temp_dir");
        let mut path_sqlite = PathBuf::from(temp_dir.path());
        path_sqlite.push("sqlite");
        let mut path_mdb = PathBuf::from(temp_dir.path());
        path_mdb.push("mdb");

        let cfg0 = StateConfig {
            path_sqlite: &*path_sqlite.to_string_lossy(),
            path_mdb: &path_mdb,
            root: &PathBuf::from("../test/gallery0"),
            n_threads: 4,
        };
        let res0 = do_scan(cfg0).await;
        assert_eq!(
            res0.thumb_keys,
            vec![
                "/a-pair-of-rams-in-the-winter_800.jpg",
                "/antelope-with-horns_800.jpg",
                "/folderA/deer-at-mcdonald-creek_800.jpg",
                "/folderB/deer-in-the-wild_800.jpg",
                "/folderB/duck-and-ducklings-in-the-pool_800.jpg",
                "/folderB/sub/first-lady-melania-trump-on-safari-looking-at-zebras_800.jpg",
                "/folderB/sub/sub/grizzly-bear-in-vast-wilderness_800.jpg",
                "/folderD/groups-of-elephants-with-babies_800.jpg",
                "/photo.jpg",
            ]
        );

        assert_eq!(
            res0.folders,
            vec![
                folder!("/", ".", 0, None),
                folder!("/folderA", "folderA", 1635097823, Some("/")),
                folder!("/folderB", "folderB", 1635097963, Some("/")),
                folder!("/folderB/sub", "sub", 1635097976, Some("/folderB")),
                folder!("/folderB/sub/sub", "sub", 1635097976, Some("/folderB/sub")),
                folder!("/folderD", "folderD", 1635101039, Some("/")),
            ]
        );

        // for m in &res0.media {
        //     println!(
        //         r#"image!("{}", "{}", "{}", {}, {}),"#,
        //         m.path, m.name, m.dir, m.mtime, m.timestamp
        //     );
        // }
        #[rustfmt::skip]
        assert_eq!(
            res0.media,
            vec![
                image!("/a-pair-of-rams-in-the-winter_800.jpg", "a-pair-of-rams-in-the-winter_800.jpg", "/", 1635097425, 1635097425),
                image!("/antelope-with-horns_800.jpg", "antelope-with-horns_800.jpg", "/", 1635098049, 1503415812),
                image!("/folderA/deer-at-mcdonald-creek_800.jpg", "deer-at-mcdonald-creek_800.jpg", "/folderA", 1635097823, 1553969835),
                image!("/folderB/deer-in-the-wild_800.jpg", "deer-in-the-wild_800.jpg", "/folderB", 1635097861, 1510497678),
                image!("/folderB/duck-and-ducklings-in-the-pool_800.jpg", "duck-and-ducklings-in-the-pool_800.jpg", "/folderB", 1635097870, 1528563996),
                image!("/folderB/sub/first-lady-melania-trump-on-safari-looking-at-zebras_800.jpg", "first-lady-melania-trump-on-safari-looking-at-zebras_800.jpg",
                 "/folderB/sub", 1635097880, 1538708222),
                image!("/folderB/sub/sub/grizzly-bear-in-vast-wilderness_800.jpg", "grizzly-bear-in-vast-wilderness_800.jpg", "/folderB/sub/sub", 1635097926, 1533470497),
                image!("/folderD/groups-of-elephants-with-babies_800.jpg", "groups-of-elephants-with-babies_800.jpg", "/folderD", 1635101081, 1635101081),
                image!("/photo.jpg", "photo.jpg", "/", 1635097492, 1635097492),
            ]
        );

        let cfg1 = StateConfig {
            path_sqlite: &*path_sqlite.to_string_lossy(),
            path_mdb: &path_mdb,
            root: &PathBuf::from("../test/gallery1"),
            n_threads: 4,
        };
        let res1 = do_scan(cfg1).await;
        assert_eq!(
            res1.thumb_keys,
            vec![
                "/antelope-with-horns_800.jpg",
                "/baby-goats-close-up_800.jpg",
                "/folderA/deer-at-mcdonald-creek_800.jpg",
                "/folderD/groups-of-elephants-with-babies_800.jpg",
                "/folderD/horses-on-the-hillside_800.jpg",
                "/photo.jpg",
            ]
        );

        assert_eq!(
            res1.folders,
            vec![
                folder!("/", ".", 0, None),
                folder!("/folderA", "folderA", 1635097823, Some("/")),
                folder!("/folderD", "folderD", 1635098049, Some("/")),
            ]
        );

        // for m in &res1.media {
        //     println!(
        //         r#"image!("{}", "{}", "{}", {}, {}),"#,
        //         m.path, m.name, m.dir, m.mtime, m.timestamp
        //     );
        // }
        #[rustfmt::skip]
        assert_eq!(
            res1.media,
            vec![
                image!("/antelope-with-horns_800.jpg", "antelope-with-horns_800.jpg", "/", 1635098049, 1503415812),
                image!("/baby-goats-close-up_800.jpg", "baby-goats-close-up_800.jpg", "/", 1635097458, 1516628540),
                image!("/folderA/deer-at-mcdonald-creek_800.jpg", "deer-at-mcdonald-creek_800.jpg", "/folderA", 1635097846, 1553969835),
                image!("/folderD/groups-of-elephants-with-babies_800.jpg", "groups-of-elephants-with-babies_800.jpg", "/folderD", 1635097942, 1635097942),
                image!("/folderD/horses-on-the-hillside_800.jpg", "horses-on-the-hillside_800.jpg", "/folderD", 1635101071, 1635101071),
                image!("/photo.jpg", "photo.jpg", "/", 1635097503, 1503747647),
            ]
        );

        assert_ne!(
            res0.thumb_kvs.get("/photo.jpg").unwrap(),
            res1.thumb_kvs.get("/photo.jpg").unwrap()
        );
    }

    /*
    #[async_std::test]
    async fn test_cpu() {
        // tide::log::with_level(tide::log::LevelFilter::Debug);

        let temp_dir = PathBuf::from("/tmp/gallerina");
        let mut path_sqlite = PathBuf::from(&temp_dir);
        path_sqlite.push("sqlite");
        let mut path_mdb = PathBuf::from(&temp_dir);
        path_mdb.push("mdb");

        let cfg = StateConfig {
            path_sqlite: &*path_sqlite.to_string_lossy(),
            path_mdb: &path_mdb,
            root: &PathBuf::from("/tmp/rots"),
            n_threads: 4,
        };
        let state = Storage::new(&cfg).await.expect("Storage::new");
        let stats = Arc::new(RwLock::new(Stats::new()));
        let scan_dir = scan_dir(stats.clone(), cfg.root.clone())
            .await
            .expect("scan_dir");
        println!("scan_dir complete");
        let (indexer, mut indexer_handle) = Indexer::start(state.clone(), stats, 12);
        indexer.update(&scan_dir).await.expect("indexer.update");
        indexer_handle.wait_stop().await;
    }
    */
}
