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

// #[derive(Debug, Serialize, Clone, Copy)]
// pub struct Stats {
//     last_scan_start: Option<chrono::DateTime<Local>>,
//     last_scan_end: Option<chrono::DateTime<Local>>,
//     scan_folders_count: u64,
//     scan_files_count: u64,
//     scan_folders_total: u64,
//     scan_files_total: u64,
// }
//
// impl Stats {
//     pub fn new() -> Self {
//         Self {
//             last_scan_start: None,
//             last_scan_end: None,
//             scan_folders_count: 0,
//             scan_files_count: 0,
//             scan_folders_total: 0,
//             scan_files_total: 0,
//         }
//     }
// }

#[derive(Clone)]
pub struct Storage {
    pub root: PathBuf,
    pub db: SqlitePool,
    pub thumb_db: heed::Database<Str, ByteSlice>,
    pub thumb_db_env: heed::Env,
}

impl<'a> Storage {
    pub async fn new(cfg: &StateConfig<'a>) -> Result<Self, StateNewError> {
        fs::create_dir_all(cfg.path_mdb)?;

        if !sqlx::Sqlite::database_exists(cfg.path_sqlite).await? {
            sqlx::Sqlite::create_database(cfg.path_sqlite).await?;
        }

        let db = db_connection(cfg.path_sqlite).await?;

        let env = heed::EnvOpenOptions::new()
            .map_size(256 * 1024 * 1024 * 1024) // 256 GiB
            .open(cfg.path_mdb)?;
        let thumb_db: heed::Database<Str, ByteSlice> = env.create_database(None)?;

        let mut storage = Self {
            root: cfg.root.clone(),
            db: db,
            thumb_db,
            thumb_db_env: env,
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
        .execute(&mut tx)
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
        .execute(&mut tx)
        .await?;

        sqlx::query("CREATE INDEX IF NOT EXISTS dir_index ON image (dir);")
            .execute(&mut tx)
            .await?;

        sqlx::query(
            r#"INSERT OR IGNORE INTO folder (path, name, dir, mtime) VALUES ("/", ".", NULL, 0);"#,
        )
        .execute(&mut tx)
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
    ) -> Result<Vec<views::MediaDataDir>, sqlx::Error> {
        sqlx::query_as("SELECT dir, name FROM image WHERE dir LIKE ?")
            .bind(format!("{}%", dir))
            .fetch_all(&self.db)
            .await
    }

    pub async fn folder_media(
        &self,
        dir: &str,
        sort: &queries::Sort,
        reverse: bool,
    ) -> Result<Vec<views::MediaData>, sqlx::Error> {
        sqlx::query_as(&format!(
            "SELECT name FROM image WHERE dir = ? ORDER BY {order_by} {order}",
            order = if reverse { "DESC" } else { "ASC" },
            order_by = match sort {
                queries::Sort::Name => "name",
                queries::Sort::Taken => "timestamp",
                queries::Sort::Modified => "mtime",
                queries::Sort::Random => "name",
            }
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
            ORDER BY {order_by} {order}",
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
    // pub stats: Arc<RwLock<Stats>>, // TODO: Put under Arc Mutex
}

#[derive(Debug)]
pub struct StateConfig<'a> {
    pub path_sqlite: &'a str,
    pub path_mdb: &'a PathBuf,
    pub root: &'a PathBuf,
    pub n_threads: usize,
}

async fn db_connection(url: &str) -> Result<SqlitePool, sqlx::Error> {
    let mut opts = SqliteConnectOptions::from_str(url)?
        .serialized(true)
        .busy_timeout(Duration::from_secs(3600));
    opts.log_statements(LevelFilter::Debug)
        .log_slow_statements(LevelFilter::Warn, Duration::from_millis(800));
    Ok(SqlitePool::connect_with(opts).await?)
}

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
            // stats: Arc::new(RwLock::new(Stats::new())),
        })
    }
}
