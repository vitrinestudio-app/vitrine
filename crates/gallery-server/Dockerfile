# Template gallery backend (api.vitrinestudio.app), ADR 0023.
# Build from the repository root:
#   docker build -f crates/gallery-server/Dockerfile -t vitrine-gallery .
# Run: see crates/gallery-server/docker-compose.example.yml and
# docs/gallery/DEPLOY.md.

FROM rust:1.99-bookworm AS build
WORKDIR /src
COPY . .
# rust-toolchain.toml pins the exact toolchain; rustup installs it here.
RUN cargo build --release --locked -p vitrine-gallery-server --bin vitrine-gallery

FROM debian:bookworm-slim
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --system --uid 10001 --home /data gallery \
    && mkdir -p /data && chown gallery /data
COPY --from=build /src/target/release/vitrine-gallery /usr/local/bin/vitrine-gallery
USER gallery
ENV GALLERY_BIND=0.0.0.0:8080 \
    GALLERY_DATA_DIR=/data
VOLUME ["/data"]
EXPOSE 8080
HEALTHCHECK --interval=30s --timeout=3s CMD ["/bin/bash", "-c", "exec 3<>/dev/tcp/127.0.0.1/8080"]
ENTRYPOINT ["/usr/local/bin/vitrine-gallery"]
