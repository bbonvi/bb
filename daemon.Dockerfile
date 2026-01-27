FROM node AS node_builder 
WORKDIR /app
COPY client/package.json client/yarn.lock ./
RUN yarn
COPY client/ ./
RUN yarn run build

#########################

FROM rust:1-slim-trixie AS rust_builder
WORKDIR /app

RUN apt-get update && apt-get install -y --no-install-recommends \
    libstdc++-14-dev \
    libgomp1 \
    libssl-dev \
    build-essential \
    pkg-config \
    && rm -rf /var/lib/apt/lists/*

COPY src src
COPY Cargo.lock Cargo.toml .

ARG NO_HEADLESS="false"

RUN if [ "$NO_HEADLESS" = "true" ]; then \
        echo "building without chromium" && \
        cargo install --locked --no-default-features \
                --root /usr/local/ --path ./ ; \
    else \
        echo "building with chromium" && \
        cargo install --locked --root /usr/local/ --path ./ ; \
    fi

#########################

FROM debian:trixie-slim

ARG NO_HEADLESS="false"

RUN apt-get update && \
    if [ "$NO_HEADLESS" != "true" ]; then \
        apt-get install -y --no-install-recommends \
            chromium \
            libharfbuzz0b \
            libnss3 \
            libfreetype6 \
            fonts-freefont-ttf \
            ca-certificates \
            wget \
        ; \
    else \
        apt-get install -y --no-install-recommends ca-certificates wget ; \
    fi && \
    rm -rf /var/lib/apt/lists/*

COPY --from=node_builder /app/build /client/build
COPY --from=rust_builder /usr/local/bin/bb /usr/local/bin/bb

ENV RUST_LOG="info,bb=info,tower_http::trace::on_response=warn"

EXPOSE 8080

CMD ["/usr/local/bin/bb", "daemon"]

