FROM rust:1.66-alpine3.17 as build
RUN apk add --no-cache musl-dev openssl-dev
ENV RUSTFLAGS="-C target-feature=-crt-static"

COPY Cargo.toml Cargo.lock ./
RUN mkdir src && touch src/lib.rs
RUN \
  --mount=type=cache,target=/opt/target \
  --mount=type=cache,target=/usr/local/cargo/registry \
  cargo build --release --lib

COPY . .
RUN \
  --mount=type=cache,target=/opt/target \
  --mount=type=cache,target=/usr/local/cargo/registry \
  cargo clean -r -p matrix-chatgpt && cargo install --locked --path .

FROM alpine:3.17
RUN apk add --no-cache musl-dev openssl-dev libgcc
COPY --from=build /usr/local/cargo/bin/matrix-chatgpt /usr/local/bin/
CMD ["/usr/local/bin/matrix-chatgpt"]
