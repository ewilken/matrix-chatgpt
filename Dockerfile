FROM rust:1.66-alpine3.17 as build
RUN apk add --no-cache musl-dev openssl-dev git
ENV RUSTFLAGS="-C target-feature=-crt-static"

COPY . .
# to work around https://github.com/rust-lang/cargo/issues/10781
RUN cargo build --config net.git-fetch-with-cli=true --locked --release 
RUN cargo install --locked --path .

FROM alpine:3.17
RUN apk add --no-cache musl-dev openssl-dev libgcc
COPY --from=build /usr/local/cargo/bin/matrix-chatgpt /usr/local/bin/
CMD ["/usr/local/bin/matrix-chatgpt"]
