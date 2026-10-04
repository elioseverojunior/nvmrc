# syntax=docker/dockerfile:1
# The release image: the static musl binaries of a release, nothing else.
# Built from prebuilt binaries only (no RUN), so both platforms build on any
# host without emulation. The context is laid out by `mise run release:image`
# as linux/<arch>/{nvmrc,nvm,nvm-exec}.
# Pinned by version tag, not digest: a new Alpine patch release is a deliberate
# bump here. The release job runs the image it pushed, by digest, before
# signing and attesting that digest.
FROM docker.io/library/alpine:3.24.2 AS certificates

FROM scratch
ARG TARGETPLATFORM
# TLS roots for `nvm ls-remote` and `nvm install` downloads.
COPY --from=certificates /etc/ssl/certs/ca-certificates.crt /etc/ssl/certs/ca-certificates.crt
COPY $TARGETPLATFORM/nvmrc $TARGETPLATFORM/nvm $TARGETPLATFORM/nvm-exec /usr/bin/
ENV NVM_DIR=/nvm
USER 65534:65534
