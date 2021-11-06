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

#[cfg(test)]
mod test {
    use async_std::task;
    use log::LevelFilter;
    use sqlx::migrate::MigrateDatabase;
    use sqlx::sqlite::{SqliteConnectOptions, SqlitePool};
    use sqlx::ConnectOptions;
    use std::path::PathBuf;
    use std::str::FromStr;
    use std::time::Duration;
    use tempfile::TempDir;

    async fn db_connection(url: &str) -> Result<SqlitePool, sqlx::Error> {
        let mut opts = SqliteConnectOptions::from_str(url)?;
        opts.log_statements(LevelFilter::Debug)
            .log_slow_statements(LevelFilter::Warn, Duration::from_millis(800));
        Ok(SqlitePool::connect_with(opts).await?)
    }

    const TX_SIZE: usize = 1024;
    async fn work(db: SqlitePool, id: usize, max: usize) {
        loop {
            for start in (id..id + max).step_by(TX_SIZE) {
                let mut tx = db.begin().await.unwrap();
                for i in start..start + TX_SIZE {
                    let s = format!("{}", id + i);
                    sqlx::query("INSERT INTO test (one, two, three) VALUES (?, ?, ?)")
                        .bind(&s)
                        .bind(&s)
                        .bind(&s)
                        .execute(&mut tx)
                        .await
                        .unwrap();
                }
                tx.commit().await.unwrap();
            }
        }
    }

    #[async_std::test]
    async fn test_sqlx1() -> Result<(), sqlx::Error> {
        let temp_dir = TempDir::new().expect("new temp_dir");
        let mut path_sqlite = PathBuf::from(temp_dir.path());
        path_sqlite.push("db.sqlite");

        sqlx::Sqlite::create_database(&*path_sqlite.to_string_lossy()).await?;
        let db = db_connection(&*path_sqlite.to_string_lossy()).await?;

        let mut tx = db.begin().await?;
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS test (
                one             TEXT PRIMARY KEY,
                two             TEXT NOT NULL,
                three           TEXT NOT NULL
            );
            "#,
        )
        .execute(&mut tx)
        .await?;
        tx.commit().await?;

        const NUM_TASKS: usize = 8;
        const MAX: usize = 1024 * 1024 * 1024;
        let mut tasks = Vec::new();
        for i in 0..NUM_TASKS {
            tasks.push(task::spawn(work(db.clone(), i * MAX, MAX)));
        }

        while let Some(task) = tasks.pop() {
            task.await;
        }

        Ok(())
    }

    use rand;
    use rand::Rng;
    use sqlx::Connection;

    #[async_std::test]
    async fn test_sqlx2() -> Result<(), sqlx::Error> {
        let mut conn = SqliteConnectOptions::new()
            .filename(":memory:")
            .connect()
            .await?;

        sqlx::query(
            r#"
    CREATE TABLE kv (k PRIMARY KEY, v);
    CREATE INDEX idx_kv ON kv (v);
    "#,
        )
        .execute(&mut conn)
        .await?;

        let mut rng = rand::thread_rng();
        for i in 0..1_000_000 {
            if i % 1_000 == 0 {
                println!("{}", i);
            }
            let key = rng.gen_range(0..1_000);
            let value = rng.gen_range(0..1_000);
            let mut tx = conn.begin().await?;

            let exists = sqlx::query("SELECT 1 FROM kv WHERE k = ?")
                .bind(key)
                .fetch_optional(&mut tx)
                .await?;
            if exists.is_some() {
                sqlx::query("UPDATE kv SET v = ? WHERE k = ?")
                    .bind(value)
                    .bind(key)
                    .execute(&mut tx)
                    .await?;
            } else {
                sqlx::query("INSERT INTO kv(k, v) VALUES (?, ?)")
                    .bind(key)
                    .bind(value)
                    .execute(&mut tx)
                    .await?;
            }
            tx.commit().await?;
        }
        Ok(())
    }
}
