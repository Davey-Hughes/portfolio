# syntax=docker/dockerfile:1

# cargo-leptos is pinned rather than installed from `releases/latest`. It compiles
# wasm-bindgen-cli-support in, and that schema version must match the wasm-bindgen crate
# this project builds *exactly*. On `latest`, the day upstream ships a cargo-leptos built
# against a different wasm-bindgen, this image stops building — while `cargo test` stays
# green, because nothing but cargo-leptos ever compares the two.
#
# These two must be bumped TOGETHER: WASM_BINDGEN_VERSION is the wasm-bindgen release
# that CARGO_LEPTOS_VERSION was verified against, and it has to equal the `wasm-bindgen`
# pin in Cargo.toml. The builder stage asserts that against Cargo.lock instead of
# trusting this comment — see "verify the pinned pair" below.
#
# .forgejo/workflows/ci.yml's wasm job greps CARGO_LEPTOS_VERSION out of this file, so
# there is no second copy to keep in sync: change it here and CI follows.
ARG CARGO_LEPTOS_VERSION=0.3.7
ARG WASM_BINDGEN_VERSION=0.2.126

# Get started with a build env with Rust nightly
FROM rustlang/rust:nightly-alpine AS builder

# Global ARGs must be redeclared inside a stage to be visible to its RUNs.
ARG CARGO_LEPTOS_VERSION
ARG WASM_BINDGEN_VERSION

RUN apk update && \
    apk add --no-cache bash curl npm libc-dev binaryen

RUN npm install -g sass

RUN curl --proto '=https' --tlsv1.3 -LsSf \
      "https://github.com/leptos-rs/cargo-leptos/releases/download/v${CARGO_LEPTOS_VERSION}/cargo-leptos-installer.sh" | sh

WORKDIR /work
COPY . .

# Verify the pinned pair before spending minutes on a compile. Without this, drift
# surfaces at the very end of `cargo leptos build` as a bare "rust Wasm file schema
# version: X / this binary schema version: Y", which reads like a toolchain bug rather
# than two version numbers that were supposed to be edited together.
# awk, not `grep -A1`: this stage runs on busybox, whose grep cannot be relied on for
# context flags. The `$` anchor is load-bearing — without it the pattern also matches
# wasm-bindgen-futures, which is versioned 0.4.x and would read as permanent drift.
RUN set -eu; \
    locked="$(awk '/^name = "wasm-bindgen"$/ { getline; gsub(/[",]/, "", $3); print $3; exit }' Cargo.lock)"; \
    if [ "$locked" != "${WASM_BINDGEN_VERSION}" ]; then \
      echo "ERROR: wasm-bindgen / cargo-leptos pins have drifted apart." >&2; \
      echo "  Cargo.lock resolves wasm-bindgen:                 ${locked}" >&2; \
      echo "  cargo-leptos ${CARGO_LEPTOS_VERSION} was verified against: ${WASM_BINDGEN_VERSION}" >&2; \
      echo "" >&2; \
      echo "Fix: verify 'cargo leptos build --release' locally, then update the" >&2; \
      echo "CARGO_LEPTOS_VERSION / WASM_BINDGEN_VERSION ARGs at the top of this" >&2; \
      echo "Dockerfile and the wasm-bindgen pin in Cargo.toml so all three agree." >&2; \
      exit 1; \
    fi; \
    echo "pinned pair OK: cargo-leptos ${CARGO_LEPTOS_VERSION} + wasm-bindgen ${locked}"

# Compile with BuildKit cache mounts: the cargo registry, git deps, and the
# target/ dir persist across image builds, so only changed crates recompile
# (a plain build re-downloads and re-compiles every dependency every time).
# Cache mounts are build-time only and are NOT baked into the image layer, so
# the outputs are copied out to /out in this same step to survive into the image
# (the runner stage COPYs from /out, not from target/).
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/work/target \
    cargo leptos build --release -vv && \
    mkdir -p /out && \
    cp target/release/portfolio /out/ && \
    cp target/release/hash.txt /out/ && \
    cp -r target/site /out/site

FROM alpine:latest AS runner

RUN apk add --no-cache curl

WORKDIR /app

COPY --from=builder /out/portfolio /app/
COPY --from=builder /out/site /app/site
# hash-files=true emits content-hashed pkg names; the server resolves them from
# hash.txt next to the binary (current_exe dir), so it must sit alongside the bin.
COPY --from=builder /out/hash.txt /app/

# Create images directory for runtime mounting
RUN mkdir -p /app/public/images /app/public/content

ENV RUST_LOG="info"
ENV LEPTOS_SITE_ADDR="0.0.0.0:8080"
ENV LEPTOS_SITE_ROOT=./site
# Match hash-files=true so HydrationScripts/HashedStylesheet emit the hashed pkg
# names (resolved via the hash.txt copied above). Local dev opts out via .env.
ENV LEPTOS_HASH_FILES="true"
# ENV IMAGES_DIR=/app/images
# ENV GALLERY_PATH=/app/public/home

# Volume for mounting images at runtime
VOLUME ["/app/public"]

EXPOSE 8080

HEALTHCHECK --interval=30s --timeout=5s --start-period=10s --retries=3 \
  CMD curl -f http://localhost:8080/healthz || exit 1

CMD ["/app/portfolio"]
