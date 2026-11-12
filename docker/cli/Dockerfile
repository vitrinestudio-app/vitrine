# The `vitrine` command line tool as a container image for CI (GitLab CI,
# Jenkins, Buildkite, CircleCI, Azure Pipelines, …). DRAFT: not published
# yet; see integrations/README.md and docs/research/2026-11-12-ci-integrations.md.
#
# Build from the repository root:
#   docker build -f docker/cli/Dockerfile -t vitrine-cli .
# Use (the project is mounted at /work; run as your own user so exported
# files belong to you):
#   docker run --rm -u "$(id -u):$(id -g)" -v "$PWD:/work" vitrine-cli vitrine validate
#
# No ENTRYPOINT on purpose: CI systems (GitLab `script:`, Jenkins
# `docker.image().inside`, CircleCI/Buildkite docker executors) run a shell
# in the image. Fonts, hyphenation patterns, catalogs and parametric frames
# are embedded in the binary; it links only glibc + libgcc (no GUI
# libraries, no fontconfig, no keychain – store credentials come from
# VITRINE_ASC_* / VITRINE_PLAY_*).

FROM rust:1.99-bookworm AS build
WORKDIR /src
COPY . .
# rust-toolchain.toml pins the exact toolchain; rustup installs it here.
ENV CARGO_PROFILE_RELEASE_STRIP=true
RUN cargo build --release --locked -p vitrine-cli --bin vitrine

FROM debian:bookworm-slim
# ca-certificates: TLS to App Store Connect / Google Play (upload).
# jq: read the upload plan's confirm token in CI scripts.
# git: checkouts inside the container (GitLab, Jenkins agents).
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates jq git \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --create-home --uid 10001 --shell /bin/bash vitrine \
    && mkdir -p /work \
    && chown vitrine /work
COPY --from=build /src/target/release/vitrine /usr/local/bin/vitrine
# Licence of the binary; the release image should also carry the
# third-party notices of the CLI archive (see the research doc).
COPY LICENSE /usr/share/doc/vitrine/LICENSE
USER vitrine
# App data (app-wide frame packs, templates) in a known place: mount a
# volume there to provide frame packs. Never Apple's bezels: their licence
# does not allow redistribution, so CI uses parametric frames or the
# project's own frame packs (`frames/` in the project).
ENV VITRINE_APP_DIR=/home/vitrine/.vitrine \
    NO_COLOR=1
WORKDIR /work
CMD ["vitrine", "--help"]
