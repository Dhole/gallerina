FROM rust:1.56-bullseye as build-rust

RUN apt-get update \
 && apt-get -y install curl build-essential clang pkg-config libjpeg-turbo-progs libpng-dev libssl-dev

ENV MAGICK_VERSION 7.1

WORKDIR /magick
RUN curl https://download.imagemagick.org/ImageMagick/download/ImageMagick.tar.gz | tar xz \
 && cd ImageMagick-${MAGICK_VERSION}* \
 && ./configure --with-magick-plus-plus=no --with-perl=no \
 && make \
 && make install

WORKDIR /backend
COPY backend .
RUN cargo build --release

FROM node:16-alpine as build-node
WORKDIR /frontend
COPY front/package.json .
RUN npm install
COPY front .
RUN npm run build

FROM debian:bullseye-slim

RUN apt-get update \
 && apt-get -y install libx11-6 libgomp1 libjbig0 liblcms2-2 libtiff5 \
                       liblqr-1-0 libpng16-16 libdjvulibre21 libfontconfig1 \
                       libwebpmux3 libwebpdemux2 libxext6  libopenexr25 \
                       libopenjp2-7 libssl1.1 \
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

WORKDIR /app
COPY --from=build-rust /backend/target/release/backend gallerina
COPY --from=build-node /frontend/public static
RUN mkdir -p /app/db
# ENTRYPOINT /app/gallerina
ENV THREADS=${GALLERINA_THREADS:-0}
ENV RUST_LOG=${GALLERINA_LOG:-info}
ENTRYPOINT /app/gallerina --addr 0.0.0.0:8080 --sqlite /app/db/db.sqlite --mdb /app/db/mdb --root /app/media --static /app/static --threads $THREADS
