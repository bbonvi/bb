FROM rust:bookworm

RUN apt-get update && apt-get install -y --no-install-recommends \
    chromium \
    curl \
    libstdc++-12-dev \
    libgomp1 \
    build-essential \
    && rm -rf /var/lib/apt/lists/*

RUN cargo install cargo-watch --locked

WORKDIR /app
