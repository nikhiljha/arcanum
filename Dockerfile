FROM gcr.io/distroless/static:nonroot
COPY --chown=nonroot:nonroot ./arcanum /app/
EXPOSE 8080
ENTRYPOINT ["/app/arcanum"]
