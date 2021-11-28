FROM gcr.io/distroless/static:nonroot
COPY --chown=nonroot:nonroot ./target/x86_64-unknown-linux-musl/release/arcanum /app/
EXPOSE 8080
ENTRYPOINT ["/app/arcanum"]
