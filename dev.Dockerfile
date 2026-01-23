FROM rust:bookworm

RUN apt-get update && apt-get install -y --no-install-recommends \
    chromium \
    curl \
    && rm -rf /var/lib/apt/lists/*

RUN cargo install cargo-watch --locked

WORKDIR /app
