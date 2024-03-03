# syntax=docker/dockerfile:experimental

FROM rust:1.75-bullseye as build-rust

RUN apt-get update \
 && apt-get -y install curl build-essential clang pkg-config libjpeg-turbo-progs libpng-dev libssl-dev

ENV MAGICK_VERSION 7.1

WORKDIR /magick
# RUN curl https://download.imagemagick.org/archive/ImageMagick.tar.gz | tar xz
# RUN curl https://download.imagemagick.org/archive/releases/ImageMagick-7.1.1-23.tar.gz | tar xz
RUN curl https://download.imagemagick.org/archive/releases/ImageMagick-7.1.1-26.tar.gz | tar xz
RUN cd ImageMagick-${MAGICK_VERSION}* \
 && ./configure --with-magick-plus-plus=no --with-perl=no \
 && make -j 4 \
 && make install

WORKDIR /backend
RUN mkdir -p /backend/lib
COPY backend .
RUN --mount=type=cache,sharing=locked,id=cargotarget,target=/backend/target cargo build --release
RUN --mount=type=cache,sharing=locked,id=cargotarget,target=/backend/target cp /backend/target/release/backend /backend/backend

FROM node:16-alpine as build-node
WORKDIR /frontend
COPY frontend/package.json .
RUN --mount=type=cache,sharing=locked,id=cargotarget,target=/frontend/node_modules npm install
COPY frontend .
RUN --mount=type=cache,sharing=locked,id=cargotarget,target=/frontend/node_modules npm run build

FROM debian:bullseye-slim

RUN apt-get update \
 && apt-get -y install libx11-6 libgomp1 libjbig0 liblcms2-2 libtiff5 \
                       liblqr-1-0 libpng16-16 libdjvulibre21 libfontconfig1 \
                       libwebpmux3 libwebpdemux2 libxext6  libopenexr25 \
                       libopenjp2-7 libssl1.1 ffmpeg \
 && rm -rfv /var/lib/apt/lists/*
COPY --from=build-rust /usr/local/lib /usr/local/lib
ENV LD_LIBRARY_PATH=/usr/local/lib
# WORKDIR /magick
# COPY --from=build-rust /magick /magick
# RUN cd ImageMagick-${MAGICK_VERSION}* \
#  && make install
# RUN rm -r ImageMagick-${MAGICK_VERSION}*
# RUN apt-get remove -y build-essential \
#  && apt-get autoremove -y

RUN apt-get update \
 && apt-get -y install gdb curl

WORKDIR /app
COPY --from=build-rust /backend/backend gallerina
COPY --from=build-rust /backend/lib lib
COPY --from=build-node /frontend/public static
RUN chmod -R a+rX gallerina lib static
RUN mkdir -p /app/db
ENTRYPOINT RUST_LOG=${GALLERINA_LOG:-info} /app/gallerina --addr 0.0.0.0:8080 --sqlite /app/db/db.sqlite --mdb /app/db/mdb --root /app/media --static /app/static --lib_dir /app/lib --threads ${GALLERINA_THREADS:-0} --page_size ${GALLERINA_PAGE_SIZE:-4096}
