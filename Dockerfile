FROM rust:1.87-alpine AS builder
RUN apk add --no-cache musl-dev
WORKDIR /build
COPY Cargo.toml Cargo.lock ./
COPY src ./src
RUN cargo build --release

FROM alpine:3.21
COPY --from=builder /build/target/release/prometheus-ecowitt-exporter /usr/local/bin/
EXPOSE 8088/tcp
ENTRYPOINT ["prometheus-ecowitt-exporter"]
