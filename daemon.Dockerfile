FROM node AS node_builder 
WORKDIR /app
COPY client/package.json client/yarn.lock ./
RUN yarn
COPY client/ ./
RUN yarn run build

#########################

FROM rust:alpine AS rust_builder 
WORKDIR /app

RUN apk add --no-cache openssl-dev musl-dev openssl-libs-static

RUN rustup toolchain list | head -1 | cut -d" " -f1 | sed -e "s/gnu/musl/" > /target.txt

RUN rustup target add $(cat /target.txt | cut -d"-" -f2- )

COPY src src
COPY Cargo.lock Cargo.toml .

ARG NO_HEADLESS="false"

ENV CFLAGS="-fPIC" 

RUN if [ "$NO_HEADLESS" = "true" ]; then \
        echo "building without chromium" && \
        cargo install --locked --no-default-features \
                --target=$(cat /target.txt | cut -d"-" -f2-) \
                --root /usr/local/ --path ./ ; \
    else \
        echo "building with chromium" && \
        cargo install --locked --target=$(cat /target.txt | cut -d"-" -f2-) --root /usr/local/ --path ./ ; \
    fi

#########################

FROM alpine:latest

ARG NO_HEADLESS="false"

RUN if [ "$NO_HEADLESS" != "true" ]; then \
        apk add --no-cache \
            chromium \
            gcompat \
            harfbuzz \
            nss \
            freetype \
            ttf-freefont \
            && mkdir /var/cache/chromium \
            && chown root:root /var/cache/chromium; \
    fi

COPY --from=node_builder /app/build /client/build
COPY --from=rust_builder /usr/local/bin/bb /usr/local/bin/bb

ENV RUST_LOG="info,bb=info,tower_http::trace::on_response=warn"
ENV PEEKALINK_API_KEY="sk_d0yoaet9psm3g5l5x8h10nk1pw4lhey6"

EXPOSE 8080

CMD ["/usr/local/bin/bb", "daemon"]

