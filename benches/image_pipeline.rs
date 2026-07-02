//! Microbenchmarks for the per-request image transcode pipeline.
//!
//! Every `/images/compressed/<path>?width=&quality=` request that misses the
//! on-disk cache runs `process_image`: decode the original -> resize with
//! Lanczos3 (only when the source is wider than the target) -> encode lossy
//! WebP via libwebp. That transcode is the heaviest user-facing CPU work in the
//! server, and it's what these benches measure — decode, resize, and encode
//! both in isolation and as the full pipeline, over the two live presets
//! (2400px and 4000px, both at q90).
//!
//! A deterministic in-memory source image is used (no disk I/O, no RNG) so
//! results compare across runs and machines. Decode is measured from an
//! in-memory JPEG buffer, which mirrors the standard (non-JXL) decode path.
//!
//! Run: `cargo bench --features ssr --bench image_pipeline`

use std::io::Cursor;
use std::time::Duration;

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use image::DynamicImage;
use portfolio::image_cache::convert_to_webp;

// A full-frame-ish 24MP source, larger than both presets so each one actually
// exercises the downscale path (`process_image` only resizes when the source is
// wider than the target width).
const SRC_W: u32 = 6000;
const SRC_H: u32 = 4000;

// The (width, quality) presets the image endpoint accepts by default.
const PRESETS: &[(u32, u8)] = &[(2400, 90), (4000, 90)];

/// Deterministic source image: a smooth gradient plus a small high-frequency
/// term so the WebP encoder sees realistic (non-flat) entropy. No RNG, so the
/// pixels — and therefore decode/encode cost — are identical every run.
fn synthetic_source(w: u32, h: u32) -> DynamicImage {
    let mut buf = image::RgbImage::new(w, h);
    for (x, y, px) in buf.enumerate_pixels_mut() {
        let r = ((x * 255) / w) as u8;
        let g = ((y * 255) / h) as u8;
        let b = (((x + y) * 255) / (w + h)) as u8 ^ (((x ^ y) & 0x1f) as u8);
        *px = image::Rgb([r, g, b]);
    }
    DynamicImage::ImageRgb8(buf)
}

/// Encode the source to an in-memory JPEG once, to feed the decode benchmark.
fn source_jpeg(img: &DynamicImage) -> Vec<u8> {
    let mut bytes = Vec::new();
    img.write_to(&mut Cursor::new(&mut bytes), image::ImageFormat::Jpeg)
        .expect("encode source jpeg");
    bytes
}

fn resize_to(img: &DynamicImage, width: u32) -> DynamicImage {
    // Mirrors `process_image`: cap width, unbounded height, Lanczos3.
    img.resize(width, u32::MAX, image::imageops::FilterType::Lanczos3)
}

/// Decode cost, from an in-memory JPEG buffer (standard decode path).
fn bench_decode(c: &mut Criterion, jpeg: &[u8]) {
    let mut g = c.benchmark_group("decode");
    g.sample_size(20);
    g.throughput(Throughput::Elements(u64::from(SRC_W) * u64::from(SRC_H)));
    g.bench_function("jpeg_24mp", |b| {
        b.iter(|| image::load_from_memory(black_box(jpeg)).expect("decode"));
    });
    g.finish();
}

/// Lanczos3 downscale to each preset width.
fn bench_resize(c: &mut Criterion, src: &DynamicImage) {
    let mut g = c.benchmark_group("resize");
    g.sample_size(20);
    for &(width, _) in PRESETS {
        g.bench_with_input(BenchmarkId::from_parameter(width), &width, |b, &w| {
            b.iter(|| resize_to(black_box(src), w));
        });
    }
    g.finish();
}

/// WebP encode of an already-resized image at q90.
fn bench_encode(c: &mut Criterion, src: &DynamicImage) {
    let mut g = c.benchmark_group("encode_webp");
    g.sample_size(20);
    for &(width, quality) in PRESETS {
        let resized = resize_to(src, width);
        g.bench_with_input(BenchmarkId::from_parameter(width), &resized, |b, img| {
            b.iter(|| convert_to_webp(black_box(img), quality).expect("encode"));
        });
    }
    g.finish();
}

/// Full pipeline: decode (from JPEG bytes) -> resize -> encode WebP, per preset.
fn bench_transcode(c: &mut Criterion, jpeg: &[u8]) {
    let mut g = c.benchmark_group("transcode");
    g.sample_size(10);
    g.measurement_time(Duration::from_secs(12));
    g.throughput(Throughput::Elements(u64::from(SRC_W) * u64::from(SRC_H)));
    for &(width, quality) in PRESETS {
        g.bench_with_input(
            BenchmarkId::new("w_q90", width),
            &(width, quality),
            |b, &(w, q)| {
                b.iter(|| {
                    let img = image::load_from_memory(black_box(jpeg)).expect("decode");
                    let resized = resize_to(&img, w);
                    convert_to_webp(&resized, q).expect("encode")
                });
            },
        );
    }
    g.finish();
}

fn benches(c: &mut Criterion) {
    let src = synthetic_source(SRC_W, SRC_H);
    let jpeg = source_jpeg(&src);
    bench_decode(c, &jpeg);
    bench_resize(c, &src);
    bench_encode(c, &src);
    bench_transcode(c, &jpeg);
}

criterion_group!(image_pipeline, benches);
criterion_main!(image_pipeline);
