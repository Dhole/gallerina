<p align="center">
    <img src="https://github.com/Dhole/gallerina/raw/master/gallerina.png" height="128">
</p>

Gallerina is a simple self-hosted photo gallery with a focus on performance.

<p align="center">
    <img src="https://github.com/Dhole/gallerina/raw/master/screenshot1.png">
</p>

# Features

- Supports multiple image formats: `JPEG`, `PNG`, `GIF`.
- Support video file formats: `MP4`.
- Parsing of EXIF metadata to index photos by taken date.
- Sorting by: name, file modification date, taken date (EXIF) and random.
- Parallel directory scanning and thumbnail creation.
- Differential update when re-scanning the image directory.
- Recursive mode to view all images in sub-folders.
- Pagination support.

## Frontend

- Lazy loading of thumbnails in folder view
- No infinite scroll (Pagination with manual page switching)
- Mouse or keyboard media view navigation
- Full screen support.  
    - In media view, click image for fullscreen; then click left area or right area to navigate, or use the left and arrow keys.
- Background preloading of N next images and M previous images in media view
    - Cancellation of image downloads outside the N+M window
- Support for big and small screens (requires refresh when window width changes)
- Navigation history with position support when going back
- Slideshow

# How to use

The easiest way to use gallerina is via the `docker` image with
`docker-compose`.  Make sure you have `docker` and `docker-compose` installed
in your system.  Then create a file called `docker-compose.yml` with the following contents:
```
version: "3"

services:
  gallerina:
    image: dhole1/gallerina:latest
    restart: always
    init: true
    ports:
      # Choose the port where the http server will be exposed.
      - 8888:8080
    environment:
      # Choose a different value to limit the number of threads for thumbnail
      # creation.  0 will default to the number of CPU cores detected.
      - GALLERINA_THREADS=6
      - GALLERINA_LOG=debug
      - GALLERINA_PAGE_SIZE=1024
    volumes:
      # Choose a folder where the database will be stored.
      - /path/to/database:/app/db:delegated
      # Set the folder where your media is.
      - /path/to/media:/app/media:ro
```
Edit the file with your chosen parameters, and start the server with:
```
docker-compose up -d
```

Now browse `http://localhost:8888` (or with a different port if you changed
it), and you should see root folder view.  To scan and index images go to the
`Control` tab and press `Start Scan`.  In the future, if you update the media
directory, you can re-scan and index the new media pressing again the `Start
Scan` button in the `Control` tab.

# Roadmap

- [x] Add support for video media
- Add fallback thumbnail creation tool
- Add support for symbolic links (for folders and media files)
- Add search support

## Features that will not be added

- User / login / password system
- TLS support
- Other SQL database backends

# Technical details

## Backend

The backend has been developed in [Rust](https://www.rust-lang.org/), with [SQLite](https://sqlite.org/index.html) and [lmdb](http://www.lmdb.tech/doc/) as databases.  The http API has been developed with the [tide](https://github.com/http-rs/tide) web application framework.  The library used to interface with sqlite is [sqlx](https://github.com/launchbadge/sqlx), and the one for lmdb is [heed](https://github.com/Kerollmops/heed)

## Frontend

The frontend has been developed using [Svelte](https://svelte.dev/).  The UI uses the [chota CSS framework](https://jenil.github.io/chota/#!).

# License

AGPLv3 (See [LICENSE](./LICENSE))
