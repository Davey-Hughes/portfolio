# Get started with a build env with Rust nightly
FROM rustlang/rust:nightly-alpine AS builder

RUN apk update && \
    apk add --no-cache bash curl npm libc-dev binaryen

RUN npm install -g sass

RUN curl --proto '=https' --tlsv1.3 -LsSf https://github.com/leptos-rs/cargo-leptos/releases/latest/download/cargo-leptos-installer.sh | sh

WORKDIR /work
COPY . .

RUN cargo leptos build --release -vv

FROM alpine:latest AS runner

WORKDIR /app

COPY --from=builder /work/target/release/portfolio /app/
COPY --from=builder /work/target/site /app/site
# hash-files=true emits content-hashed pkg names; the server resolves them from
# hash.txt next to the binary (current_exe dir), so it must sit alongside the bin.
COPY --from=builder /work/target/release/hash.txt /app/

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

CMD ["/app/portfolio"]
