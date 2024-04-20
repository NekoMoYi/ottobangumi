FROM rust:alpine as builder
WORKDIR /app
COPY ./ /app
RUN mkdir -p /root/.cargo \
    && echo '[source.crates-io]'> /root/.cargo/config \
    && echo 'replace-with = "tuna"'> /root/.cargo/config \
    && echo '[source.tuna]'> /root/.cargo/config  \
    && echo 'registry = "https://mirrors.tuna.tsinghua.edu.cn/git/crates.io-index.git"'> /root/.cargo/config
RUN apk add --no-cache musl-dev pkgconfig openssl-dev perl make
RUN rustup target add x86_64-unknown-linux-musl
RUN cargo build --release --target=x86_64-unknown-linux-musl

FROM alpine:latest
WORKDIR /app
COPY --from=builder /app/target/release/ottobangumi .
CMD ["/app/ottobangumi"]