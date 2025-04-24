use async_std::sync::Arc;
use heed::types::*;
use log::LevelFilter;
use sqlx::migrate::MigrateDatabase;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePool};
use sqlx::ConnectOptions;
use std::error::Error;
use std::fmt;
use std::fs;
use std::io;
use std::path::PathBuf;
use std::str::FromStr;
use std::time::Duration;

use crate::models::{queries, views};
use crate::scanner::Scanner;

#[allow(unused)]
#[derive(Debug)]
pub enum ThumbError {
    Heed(heed::Error),
    NotFound,
}

impl From<heed::Error> for ThumbError {
    fn from(error: heed::Error) -> Self {
        Self::Heed(error)
    }
}

impl fmt::Display for ThumbError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ThumbError {:?}", self)
    }
}

impl Error for ThumbError {}

#[derive(Clone)]
pub struct Storage {
    pub root: PathBuf,
    pub db: SqlitePool,
    pub thumb_db: heed::Database<Str, ByteSlice>,
    pub thumb_db_env: heed::Env,
    pub page_size: usize,
}

impl<'a> Storage {
    pub async fn new(cfg: &StateConfig<'a>) -> Result<Self, StateNewError> {
        fs::create_dir_all(cfg.path_mdb)?;

        if !sqlx::Sqlite::database_exists(cfg.path_sqlite).await? {
            sqlx::Sqlite::create_database(cfg.path_sqlite).await?;
        }

        let db = db_connection(cfg.path_sqlite, cfg.lib_dir).await?;

        let env = heed::EnvOpenOptions::new()
            .map_size(256 * 1024 * 1024 * 1024) // 256 GiB
            .open(cfg.path_mdb)?;
        let thumb_db: heed::Database<Str, ByteSlice> = env.create_database(None)?;

        let mut storage = Self {
            root: cfg.root.clone(),
            db,
            thumb_db,
            thumb_db_env: env,
            page_size: cfg.page_size,
        };
        storage.init().await?;
        Ok(storage)
    }
    async fn init(&mut self) -> Result<(), sqlx::Error> {
        let mut tx = self.db.begin().await?;
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS folder (
                path  TEXT PRIMARY KEY,
                name  TEXT NOT NULL,
                dir   TEXT,
                mtime INTEGER NOT NULL,
                FOREIGN KEY(dir) REFERENCES folder(path)
            );
            "#,
        )
        .execute(&mut *tx)
        .await?;

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS image (
                path            TEXT PRIMARY KEY,
                name            TEXT NOT NULL,
                dir             TEXT NOT NULL,
                mtime           INTEGER NOT NULL,
                timestamp       INTEGER NOT NULL,
                FOREIGN KEY(dir)  REFERENCES folder(path)
            );
            "#,
        )
        .execute(&mut *tx)
        .await?;

        sqlx::query("CREATE INDEX IF NOT EXISTS dir_index ON image (dir);")
            .execute(&mut *tx)
            .await?;

        sqlx::query(
            r#"INSERT OR IGNORE INTO folder (path, name, dir, mtime) VALUES ("/", ".", NULL, 0);"#,
        )
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;

        Ok(())
    }

    pub fn root(&self) -> &PathBuf {
        &self.root
    }

    pub async fn folder_media_recursive(
        &self,
        dir: &str,
        page: usize,
        sort: &queries::Sort,
        seed: usize,
    ) -> Result<Vec<views::MediaDataDir>, sqlx::Error> {
        let sort_random = format!("hash({} || path)", seed);
        sqlx::query_as(&format!("SELECT dir, name, COUNT() OVER() AS total FROM image WHERE dir LIKE ? ORDER BY {order_by} COLLATE NOCASE ASC LIMIT {limit} OFFSET {offset}",
            order_by = match sort {
                queries::Sort::Name => "path",
                queries::Sort::Taken => "timestamp",
                queries::Sort::Modified => "mtime",
                queries::Sort::Random => &sort_random,
            },
            limit = self.page_size,
            offset = page * self.page_size,
        ))
        .bind(format!("{}%", dir))
        .fetch_all(&self.db)
        .await
    }

    pub async fn folder_media(
        &self,
        dir: &str,
        page: usize,
        sort: &queries::Sort,
        seed: usize,
        reverse: bool,
    ) -> Result<Vec<views::MediaData>, sqlx::Error> {
        let sort_random = format!("hash({} || path)", seed);
        sqlx::query_as(&format!(
            "SELECT name, COUNT() OVER() AS total FROM image WHERE dir = ? ORDER BY {order_by} COLLATE NOCASE {order} LIMIT {limit} OFFSET {offset}",
            order = if reverse { "DESC" } else { "ASC" },
            order_by = match sort {
                queries::Sort::Name => "path",
                queries::Sort::Taken => "timestamp",
                queries::Sort::Modified => "mtime",
                queries::Sort::Random => &sort_random,
            },
            limit = self.page_size,
            offset = page * self.page_size, 
        ))
        .bind(dir)
        .fetch_all(&self.db)
        .await
    }

    pub async fn folder_folders(
        &self,
        dir: &str,
        sort: &queries::Sort,
        reverse: bool,
    ) -> Result<Vec<views::FolderData>, sqlx::Error> {
        sqlx::query_as(&format!(
            "SELECT folder.name as name, MIN(image.name) as media
            FROM folder
                LEFT JOIN image ON image.dir = folder.path
            WHERE folder.dir = ?
            GROUP BY folder.name
            ORDER BY {order_by} COLLATE NOCASE {order}",
            order = if reverse { "DESC" } else { "ASC" },
            order_by = match sort {
                queries::Sort::Name => "folder.name",
                queries::Sort::Taken => "folder.mtime",
                queries::Sort::Modified => "folder.mtime",
                queries::Sort::Random => "folder.name",
            }
        ))
        .bind(dir)
        .fetch_all(&self.db)
        .await
    }

    pub fn thumb(&self, path: &str) -> Result<Vec<u8>, ThumbError> {
        let rtxn = self.thumb_db_env.read_txn()?;
        Ok(self
            .thumb_db
            .get(&rtxn, path)?
            .ok_or(ThumbError::NotFound)?
            .to_vec())
    }
}

#[derive(Clone)]
pub struct State {
    pub storage: Storage,
    pub scanner: Arc<Scanner>,
    pub cfg: Config,
    // pub stats: Arc<RwLock<Stats>>, // TODO: Put under Arc Mutex
}

#[derive(Debug, Clone)]
pub struct Config {
    pub webp_quality: usize,
    pub webp_compression: usize,
}

#[derive(Debug)]
pub struct StateConfig<'a> {
    pub path_sqlite: &'a str,
    pub lib_dir: &'a PathBuf,
    pub path_mdb: &'a PathBuf,
    pub root: &'a PathBuf,
    pub n_threads: usize,
    pub page_size: usize,
    pub webp_quality: usize,
    pub webp_compression: usize,
}

async fn db_connection(url: &str, lib_dir: &PathBuf) -> Result<SqlitePool, sqlx::Error> {
    let mut hash_path = lib_dir.clone();
    hash_path.push("hash");
    let opts = SqliteConnectOptions::from_str(url)?
        .serialized(true)
        .busy_timeout(Duration::from_secs(3600))
        .extension(hash_path.as_path().to_string_lossy().to_string())
        .log_statements(LevelFilter::Debug)
        .log_slow_statements(LevelFilter::Warn, Duration::from_millis(800));
    Ok(SqlitePool::connect_with(opts).await?)
}

#[allow(unused)]
#[derive(Debug)]
pub enum StateNewError {
    Heed(heed::Error),
    Sqlx(sqlx::Error),
    Io(io::Error),
}

impl From<heed::Error> for StateNewError {
    fn from(error: heed::Error) -> Self {
        Self::Heed(error)
    }
}

impl From<sqlx::Error> for StateNewError {
    fn from(error: sqlx::Error) -> Self {
        Self::Sqlx(error)
    }
}

impl From<io::Error> for StateNewError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

impl fmt::Display for StateNewError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "StateNewError {:?}", self)
    }
}

impl Error for StateNewError {}

impl<'a> State {
    pub async fn new(cfg: &StateConfig<'a>) -> Result<Self, StateNewError> {
        let storage = Storage::new(cfg).await?;
        Ok(Self {
            storage: storage.clone(),
            scanner: Arc::new(Scanner::new(storage, cfg.n_threads)),
            cfg: Config {
                webp_quality: cfg.webp_quality,
                webp_compression: cfg.webp_compression,
            }
            // stats: Arc::new(RwLock::new(Stats::new())),
        })
    }
}
