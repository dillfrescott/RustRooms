FROM rust AS builder

RUN apt-get update && apt-get install -y git

COPY . /usr/src/rustrooms
WORKDIR /usr/src/rustrooms

RUN cargo build --release

FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y libssl-dev ca-certificates && rm -rf /var/lib/apt/lists/*

COPY --from=builder /usr/src/rustrooms/target/release/rust_rooms /usr/local/bin/rust_rooms

CMD ["rust_rooms"]