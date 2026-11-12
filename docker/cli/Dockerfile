# The `vitrine` command line tool as a container image for CI (GitLab CI,
# Jenkins, Buildkite, CircleCI, Azure Pipelines, …). Published as
# ghcr.io/vitrinestudio-app/vitrine-cli by .github/workflows/release.yml.
# Two variants (build targets) in this file:
#
# 1. `release` – what the release workflow publishes: Alpine + the static
#    musl `vitrine` binary from the release archive
#    (vitrine-cli_linux_<x86_64|aarch64>-musl.tar.gz) and its licence files.
#    The workflow unpacks the archive for the image's architecture into
#    dist/image/ (vitrine + licenses/) and runs
#      docker buildx build --platform linux/amd64 --target release -f docker/cli/Dockerfile .
#    Locally, from a downloaded release:
#      mkdir -p dist/image && tar xzf vitrine-cli_linux_x86_64-musl.tar.gz -C dist/image --strip-components=1
#      docker build --target release -f docker/cli/Dockerfile -t vitrine-cli .
#
# 2. `source` (the default, last stage) – builds the CLI from this checkout
#    (glibc, Debian), for local images without a release:
#      docker build -f docker/cli/Dockerfile -t vitrine-cli .
#
# Use (the project is mounted at /work; run as your own user so exported
# files belong to you):
#   docker run --rm -u "$(id -u):$(id -g)" -v "$PWD:/work" vitrine-cli vitrine validate
#
# No ENTRYPOINT on purpose: CI systems (GitLab `script:`, Jenkins
# `docker.image().inside`, CircleCI/Buildkite docker executors) run a shell
# in the image (POSIX `sh` in `release`, also `bash` in `source`). Fonts,
# hyphenation patterns, catalogs and parametric frames are embedded in the
# binary (no fontconfig, no GUI libraries, no keychain – store credentials
# come from VITRINE_ASC_* / VITRINE_PLAY_*). Both variants: ca-certificates
# (TLS to App Store Connect / Google Play), jq (read the upload plan's
# confirm token in CI scripts), git (checkouts inside the container),
# non-root user `vitrine` (uid 10001), VITRINE_APP_DIR in its home: mount a
# volume there to provide frame packs. Never Apple's bezels: their licence
# does not allow redistribution, so CI uses parametric frames or the
# project's own frame packs (`frames/` in the project).

# ---- release: Alpine + the static binary of the release archive ----
FROM alpine:3.24 AS release
RUN apk add --no-cache ca-certificates jq git \
    && adduser -D -u 10001 -s /bin/sh vitrine \
    && mkdir -p /work \
    && chown vitrine /work
COPY --chmod=0755 dist/image/vitrine /usr/local/bin/vitrine
# LICENSE.txt, THIRD-PARTY-NOTICES.html, font and hyphenation licences.
COPY dist/image/licenses /usr/share/doc/vitrine/licenses
USER vitrine
ENV VITRINE_APP_DIR=/home/vitrine/.vitrine \
    NO_COLOR=1
WORKDIR /work
CMD ["vitrine", "--help"]

# ---- source: build from the checkout (default target) ----
FROM rust:1.99-bookworm AS build
WORKDIR /src
COPY . .
# rust-toolchain.toml pins the exact toolchain; rustup installs it here.
ENV CARGO_PROFILE_RELEASE_STRIP=true
RUN cargo build --release --locked -p vitrine-cli --bin vitrine

FROM debian:bookworm-slim AS source
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates jq git \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --create-home --uid 10001 --shell /bin/bash vitrine \
    && mkdir -p /work \
    && chown vitrine /work
COPY --from=build /src/target/release/vitrine /usr/local/bin/vitrine
# Only the project's own licence: third-party notices come with the release
# archive (scripts/release/third-party-notices.sh needs cargo-about).
COPY LICENSE /usr/share/doc/vitrine/LICENSE
USER vitrine
ENV VITRINE_APP_DIR=/home/vitrine/.vitrine \
    NO_COLOR=1
WORKDIR /work
CMD ["vitrine", "--help"]
