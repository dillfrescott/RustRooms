FROM rust:1-bookworm AS builder
# cmake is required to build aws-lc-sys (BoringSSL), which the embedded
# turn-server depends on.
RUN apt-get update && apt-get install -y git cmake && rm -rf /var/lib/apt/lists/*
WORKDIR /usr/src/rustrooms
COPY . .
RUN cargo build --release

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates libssl3 && rm -rf /var/lib/apt/lists/*
COPY --from=builder /usr/src/rustrooms/target/release/rust_rooms /rust_rooms
CMD ["/rust_rooms"]
