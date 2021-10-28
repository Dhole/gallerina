# Libraries

- Hash algorithm: [blake3](https://crates.io/crates/blake3)
- sql library:
    - a. [sqlx](https://github.com/launchbadge/sqlx) (async (runtime agnostic))
    - b. [rusqlite](https://docs.rs/rusqlite/0.25.3/rusqlite)
- key-value library (for thumbnails):
    - a. sqlite
    - b. lmdb with https://github.com/Kerollmops/heed
    - c. lmdb with https://github.com/mozilla/rkv
    - d. sled https://github.com/spacejam/sled
- Thumbnails format
    - a. webp with https://github.com/jaredforth/webp (WIP, probably slow)
    - b. jpeg with https://github.com/image-rs/image (regular speed)
    - c. jpeg with https://github.com/imageoptim/mozjpeg-rust (fastest option)
- Exif parsing
    - a. https://github.com/kamadak/exif-rs (pure rust)
    - b. https://github.com/felixc/rexiv2 (binding to GObject library)
    - c. https://github.com/exif-js/exif-js (plot twist: parse exif from the client)
- http framework
    - a. https://github.com/actix/actix-web (async tokio)
    - b. https://github.com/http-rs/tide (async async-std)
    - c. https://github.com/seanmonstar/warp (async tokio)
---

# SQL Schema

```sql
CREATE TABLE folder (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    path TEXT,
    FOREIGN KEY(path) REFERENCES folder(id),
);
CREATE TABLE image (
    id              TEXT PRIMARY KEY,
    filename        TEXT NOT NULL,
    hash            BLOB NOT NULL,
    timestamp       INTEGER NOT NULL,
    mtime           INTEGER NOT NULL,
    exif            BLOB NOT NULL
    path            TEXT NOT NULL,
    FOREIGN KEY(path)  REFERENCES folder(id) NOT NULL,
);
```

# Benchmarks

## sqlite insert A

with --debug
```rust
struct Image {
    pathfilename: String,
    path: String,
    filename: String,
    hash: Vec<u8>, // 32 bytes
    timestamp: u64,
    mtime: u64,
    exif: Vec<u8>, // 256 bytes
}
```

Insert 1M with batches of 1k takes ~5minutes.

## JPEG thumbnail

with --release

Decode JPEG and resize to 512x512 takes ~100ms (files via NFS on gigabit ethernet).

##  JPEG thumbnail + store in kv-db

Read ~5k images, generated thumbnails 512x512 and store them in key-value db: 17m
~200ms per image.

Test with 1M images, same configuration as before, took 11h 11m

# TODO

- [ ] Video thumbnails
- [ ] Image thumbnail fallback (for when JpegDecoder::new fails)
- [x] Folder thumnail
- [x] Ignore hidden files

## Folder thumbnail

Option A
```
SELECT folder.name, MIN(image.name)
FROM folder 
    INNER JOIN image 
        ON image.dir = folder.path
WHERE folder.dir = "/path" 
GROUP BY folder.name;
```

Option B (Slower than A)
```
SELECT fo.name, im.name
FROM folder fo
    INNER JOIN image im
        ON im.dir = fo.path
    INNER JOIN
    (
        SELECT path, MAX(name) maxval
        FROM image
        GROUP BY dir
    ) im2 ON im.path = im2.path AND
            im.name = im2.maxval
WHERE fo.dir = "/path";
```

Option C
```
SELECT fo.name, im.name 
FROM folder AS fo LEFT JOIN (
    SELECT *, ROW_NUMBER() OVER(PARTITION BY dir ORDER BY name DESC) AS RowNo
    FROM image  
) AS im ON im.path = fo.path AND im.RowNo=1
WHERE fo.dir = "/path";
```
