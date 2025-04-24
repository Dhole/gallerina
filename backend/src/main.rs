use http_types::headers::HeaderValue;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::str::FromStr;
use structopt::StructOpt;
use tide::security::{CorsMiddleware, Origin};

mod exif;
mod ffmpeg;
mod magick;
mod models;
mod routes;
mod scanner;
mod state;
mod utils;

#[cfg(feature = "dhat-heap")]
#[global_allocator]
static ALLOC: dhat::Alloc = dhat::Alloc;

// #[derive(Debug, Deserialize, Serialize)]
// struct Image {
//     pathfilename: String,
//     filename: String,
//     hash: Vec<u8>,
//     timestamp: u64,
//     mtime: u64,
//     exif: Vec<u8>,
//     path: String,
// }

/// Http demo
#[derive(StructOpt, Debug)]
#[structopt(name = "gallerina")]
struct Args {
    /// SQLite Database file to use to index folders and media
    #[structopt(long)]
    sqlite: String,

    /// MDB Database path to use to store thumbnails
    #[structopt(long, parse(from_os_str))]
    mdb: PathBuf,

    /// Root directory with the media
    #[structopt(long, parse(from_os_str))]
    root: PathBuf,

    /// Address to listen to
    #[structopt(long, parse(try_from_str = SocketAddr::from_str),
        default_value = "127.0.0.1:8080")]
    addr: SocketAddr,

    /// Number of threads for thumbnail creation
    #[structopt(long, default_value = "0")]
    threads: usize,

    /// Static directory which will be served at the root http path
    #[structopt(long = "static", parse(from_os_str))]
    static_dir: Option<PathBuf>,

    /// Directory with sqlite3 hash dynamic library
    #[structopt(long = "lib_dir", parse(from_os_str))]
    lib_dir: Option<PathBuf>,

    /// Number of images per page
    #[structopt(long = "page_size", default_value = "4096")]
    page_size: usize,

    /// webp compression quality, from 0 to 100
    #[structopt(long = "webp_quality", default_value = "85")]
    webp_quality: usize,

    /// webp compression speed/size, from 0 to 6
    #[structopt(long = "webp_compression", default_value = "4")]
    webp_compression: usize,
}

#[async_std::main]
async fn main() -> tide::Result<()> {
    #[cfg(feature = "dhat-heap")]
    static PROFILER: std::sync::OnceLock<std::sync::Mutex<Option<dhat::Profiler>>> =
        std::sync::OnceLock::new();
    #[cfg(feature = "dhat-heap")]
    {
        PROFILER
            .set(std::sync::Mutex::new(Some(dhat::Profiler::new_heap())))
            .unwrap();
        ctrlc::set_handler(|| {
            println!("Ctrl-C, dropping profiler...");
            let profiler = PROFILER.get().unwrap().lock().unwrap().take();
            drop(profiler);
            std::process::exit(0);
        })
        .expect("Error setting Ctrl-C handler");
    }

    env_logger::init();
    let args = Args::from_args();
    // tide::log::with_level(tide::log::LevelFilter::Info);

    let n_threads = if args.threads == 0 {
        num_cpus::get()
    } else {
        args.threads
    };
    assert!(args.webp_quality <= 100);
    assert!(args.webp_compression <= 6);

    let state = state::State::new(&state::StateConfig {
        path_sqlite: &args.sqlite,
        lib_dir: &args.lib_dir.unwrap_or(PathBuf::from("./lib")),
        path_mdb: &args.mdb,
        root: &args.root,
        n_threads,
        page_size: args.page_size,
        webp_quality: args.webp_quality,
        webp_compression: args.webp_compression,
    })
    .await?;
    let mut app = tide::with_state(state);

    app.with(
        CorsMiddleware::new()
            .allow_methods("GET, POST, OPTIONS".parse::<HeaderValue>().unwrap())
            .allow_origin(Origin::from("*"))
            .allow_credentials(false),
    );
    app.with(tide::log::LogMiddleware::new());

    if let Some(static_dir) = args.static_dir {
        let mut index = static_dir.clone();
        index.push("index.html");
        app.at("/").serve_file(&index)?;
        app.at("/").serve_dir(&static_dir)?;
    }

    app.at("/api/folder").get(routes::get_folder);
    app.at("/api/folderRecursive")
        .get(routes::get_folder_recursive);
    app.at("/api/thumb").get(routes::get_thumb);
    app.at("/api/src/:name").get(routes::get_src);
    app.at("/api/raw/:name").get(routes::get_raw);
    app.at("/api/status").get(routes::get_status);
    app.at("/api/scanner/run").post(routes::post_scan_run);
    app.at("/api/scanner/stop").post(routes::post_scan_stop);
    app.listen(args.addr).await?;
    Ok(())
}
