FROM rust:1-slim-trixie

RUN apt-get update && apt-get install -y --no-install-recommends \
    chromium \
    curl \
    libstdc++-14-dev \
    libgomp1 \
    libssl-dev \
    build-essential \
    pkg-config \
    clang \
    mold \
    && rm -rf /var/lib/apt/lists/*

RUN cargo install cargo-watch --locked

ENV RUSTFLAGS="-C link-arg=-fuse-ld=mold"
ENV CC=clang

WORKDIR /app
