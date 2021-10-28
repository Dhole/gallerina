use http_types::headers::HeaderValue;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::str::FromStr;
use structopt::StructOpt;
use tide::security::{CorsMiddleware, Origin};

mod exif;
mod models;
mod routes;
mod scanner;
mod state;
mod utils;

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
    #[structopt(short, long)]
    sqlite: String,

    /// MDB Database path to use to store thumbnails
    #[structopt(short, long, parse(from_os_str))]
    mdb: PathBuf,

    /// Root directory with the media
    #[structopt(short, long, parse(from_os_str))]
    root: PathBuf,

    /// Address to listen to
    #[structopt(short, long, parse(try_from_str = SocketAddr::from_str),
        default_value = "127.0.0.1:8080")]
    addr: SocketAddr,

    /// Number of threads for thumbnail creation
    #[structopt(short, long, default_value = "0")]
    threads: usize,

    /// Static directory which will be served at the root http path
    #[structopt(short = "static", long = "static", parse(from_os_str))]
    static_dir: Option<PathBuf>,
}

#[async_std::main]
async fn main() -> tide::Result<()> {
    env_logger::init();
    let args = Args::from_args();
    // tide::log::with_level(tide::log::LevelFilter::Info);

    let n_threads = if args.threads == 0 {
        num_cpus::get()
    } else {
        args.threads
    };

    let state = state::State::new(&state::StateConfig {
        path_sqlite: &args.sqlite,
        path_mdb: &args.mdb,
        root: &args.root,
        n_threads: n_threads,
    })
    .await?;
    let mut app = tide::with_state(state);

    app.with(
        CorsMiddleware::new()
            .allow_methods("GET, POST, OPTIONS".parse::<HeaderValue>().unwrap())
            .allow_origin(Origin::from("*"))
            .allow_credentials(false),
    );

    if let Some(static_dir) = args.static_dir {
        let mut index = static_dir.clone();
        index.push("index.html");
        app.at("/").serve_file(&index)?;
        app.at("/").serve_dir(&static_dir)?;
    }

    app.at("/api/folder").get(routes::get_folder);
    app.at("/api/thumb").get(routes::get_thumb);
    app.at("/api/src/:name").get(routes::get_src);
    app.at("/api/status").get(routes::get_status);
    app.at("/api/scanner/run").post(routes::post_scan_run);
    app.at("/api/scanner/stop").post(routes::post_scan_stop);
    app.listen(args.addr).await?;
    Ok(())
}
