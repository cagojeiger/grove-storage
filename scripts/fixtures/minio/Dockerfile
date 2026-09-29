# Test-only build of RELEASE.2025-09-07T16-13-09Z; no vendor image registry required.
FROM golang:1.24.7-bookworm@sha256:b8bae5bd9ba9b1f89b635c91c24cc75cea335a16fb5076310f38566fc674b1ec AS build
ENV CGO_ENABLED=0 GOTOOLCHAIN=local
RUN go install github.com/minio/minio@07c3a429bfed433e49018cb0f78a52145d4bedeb

FROM scratch
LABEL org.opencontainers.image.source="https://github.com/minio/minio" \
      org.opencontainers.image.revision="07c3a429bfed433e49018cb0f78a52145d4bedeb" \
      org.opencontainers.image.licenses="AGPL-3.0-only"
COPY --from=build /etc/ssl/certs/ca-certificates.crt /etc/ssl/certs/ca-certificates.crt
COPY --from=build /go/bin/minio /minio
COPY --from=build /go/pkg/mod/github.com/minio/minio@*/LICENSE /LICENSE
VOLUME /data
ENTRYPOINT ["/minio"]
