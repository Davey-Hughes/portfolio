# Get started with a build env with Rust nightly
FROM rustlang/rust:nightly-alpine AS builder

RUN apk update && \
    apk add --no-cache bash curl npm libc-dev binaryen

RUN npm install -g sass

RUN curl --proto '=https' --tlsv1.3 -LsSf https://github.com/leptos-rs/cargo-leptos/releases/latest/download/cargo-leptos-installer.sh | sh

# Add the WASM target
RUN rustup target add wasm32-unknown-unknown

WORKDIR /work
COPY . .

RUN cargo leptos build --release -vv

FROM rustlang/rust:nightly-alpine AS runner

WORKDIR /app

COPY --from=builder /work/target/release/portfolio /app/
COPY --from=builder /work/target/site /app/site
COPY --from=builder /work/Cargo.toml /app/

# Create images directory for runtime mounting
RUN mkdir -p /app/images/gallery /app/images/categories

ENV RUST_LOG="info"
ENV LEPTOS_SITE_ADDR="0.0.0.0:8080"
ENV LEPTOS_SITE_ROOT=./site
ENV IMAGES_DIR=/app/images
ENV GALLERY_PATH=/app/images/gallery
ENV CATEGORIES_PATH=/app/images/categories

# Volume for mounting images at runtime
VOLUME ["/app/images"]

EXPOSE 8080

CMD ["/app/portfolio"]
