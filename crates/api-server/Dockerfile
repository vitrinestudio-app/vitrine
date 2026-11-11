# Backend of api.vitrinestudio.app (template gallery, GitHub login), ADR 0023/0025.
# Build from the repository root:
#   docker build -f crates/api-server/Dockerfile -t vitrine-api .
# Run: see crates/api-server/docker-compose.example.yml and docs/api/DEPLOY.md.

FROM rust:1.99-bookworm AS build
WORKDIR /src
COPY . .
# rust-toolchain.toml pins the exact toolchain; rustup installs it here.
RUN cargo build --release --locked -p vitrine-api-server --bin vitrine-api

FROM debian:bookworm-slim
# ca-certificates: TLS to github.com / api.github.com (GitHub login).
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --system --uid 10001 --home /data vitrine \
    && mkdir -p /data && chown vitrine /data
COPY --from=build /src/target/release/vitrine-api /usr/local/bin/vitrine-api
USER vitrine
ENV VITRINE_API_BIND=0.0.0.0:8080 \
    VITRINE_API_DATA_DIR=/data
VOLUME ["/data"]
EXPOSE 8080
HEALTHCHECK --interval=30s --timeout=3s CMD ["/bin/bash", "-c", "exec 3<>/dev/tcp/127.0.0.1/8080"]
ENTRYPOINT ["/usr/local/bin/vitrine-api"]
