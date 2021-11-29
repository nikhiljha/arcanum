FROM clux/muslrust:1.56.1 as builder

WORKDIR ./volume
COPY ./Cargo.toml ./Cargo.toml
COPY ./Cargo.lock ./Cargo.lock
COPY ./src ./src
RUN cargo build

FROM alpine:3.14
RUN apk add --no-cache musl musl-dev openssl openssl-dev
COPY --from=builder /volume/volume/target/x86_64-unknown-linux-musl/debug/arcanum /app/
EXPOSE 8080
CMD ["/app/arcanum"]
