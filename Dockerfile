FROM clux/muslrust:1.56.1 as builder

WORKDIR ./volume
COPY ./Cargo.toml ./Cargo.toml
COPY ./Cargo.lock ./Cargo.lock
COPY ./src ./src
RUN cargo build --release

FROM gcr.io/distroless/static:nonroot
COPY --from=builder /volume/volume/target/x86_64-unknown-linux-musl/release/arcanum /app/
EXPOSE 8080
CMD ["/app/arcanum"]
