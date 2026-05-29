# Get started with a build env with Rust nightly
FROM rustlang/rust:nightly-alpine AS builder

RUN apk update && \
    apk add --no-cache bash curl npm libc-dev binaryen

RUN npm install -g sass

RUN curl --proto '=https' --tlsv1.3 -LsSf https://github.com/leptos-rs/cargo-leptos/releases/latest/download/cargo-leptos-installer.sh | sh

WORKDIR /work
COPY . .

RUN cargo leptos build --release -vv

FROM rustlang/rust:nightly-alpine AS runner

WORKDIR /app

COPY --from=builder /work/target/release/portfolio /app/
COPY --from=builder /work/target/site /app/site

# Create images directory for runtime mounting
RUN mkdir -p /app/public/images /app/public/content

ENV RUST_LOG="info"
ENV LEPTOS_SITE_ADDR="0.0.0.0:8080"
ENV LEPTOS_SITE_ROOT=./site
# ENV IMAGES_DIR=/app/images
# ENV GALLERY_PATH=/app/public/home

# Volume for mounting images at runtime
VOLUME ["/app/public"]

EXPOSE 8080

CMD ["/app/portfolio"]
