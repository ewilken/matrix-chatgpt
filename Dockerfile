FROM rust:1.66-alpine3.17 as build
COPY ./ ./
RUN cargo install --locked --path .

FROM alpine:3.17
COPY --from=build /usr/local/cargo/bin/matrix-chatgpt /usr/local/bin/
CMD ["/usr/local/bin/matrix-chatgpt"]
