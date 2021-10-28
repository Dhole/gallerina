FROM rust:1.56-alpine as build-rust
WORKDIR /backend
RUN apk add musl-dev openssl-dev
COPY backend .
RUN cargo build --release

FROM node:16-alpine as build-node
WORKDIR /frontend
COPY front/package.json .
RUN npm install
COPY front .
RUN npm run build

FROM alpine:3.14
WORKDIR /app
COPY --from=build-rust /backend/target/release/backend gallerina
COPY --from=build-node /frontend/public static
RUN mkdir -p /app/db
# ENTRYPOINT /app/gallerina
ENV THREADS=${GALLERINA_THREADS:-0}
ENV RUST_LOG=${GALLERINA_LOG:-info}
ENTRYPOINT /app/gallerina --addr 0.0.0.0:8080 --sqlite /app/db/db.sqlite --mdb /app/db/mdb --root /app/media --static /app/static --threads $THREADS
