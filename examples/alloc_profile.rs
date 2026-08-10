//! Allocation profile for one image transcode.
//!
//! Timing benches (`cargo bench`) don't surface allocations, but the transcode
//! decodes a full-res image, allocates a resized buffer, and re-buffers it as
//! RGB/RGBA for libwebp — so allocations, not just cycles, are a lever. This
//! installs dhat as the global allocator, runs one decode -> resize -> WebP
//! encode over a deterministic in-memory source, and prints total allocations +
//! bytes: a hard, re-runnable number to verify allocation-reduction work
//! against. Also writes `dhat-heap.json` for the dhat viewer
//! (<https://nnethercote.github.io/dh_view/dh_view.html>).
//!
//! Run: `cargo run --release --example alloc_profile --features ssr`
//!      `cargo run --release --example alloc_profile --features ssr 4000`  (custom width)

use std::io::Cursor;

use image::DynamicImage;
use portfolio::image_cache::{convert_to_webp, resize_for_width};

#[global_allocator]
static ALLOC: dhat::Alloc = dhat::Alloc;

const SRC_W: u32 = 6000;
const SRC_H: u32 = 4000;

/// Deterministic source (matches `benches/image_pipeline.rs`): gradient + a
/// small high-frequency term, no RNG.
#[expect(
    clippy::many_single_char_names,
    reason = "x/y/w/h and r/g/b are the conventional names for pixel coordinates and channels"
)]
fn synthetic_source(w: u32, h: u32) -> DynamicImage {
    let mut buf = image::RgbImage::new(w, h);
    for (x, y, px) in buf.enumerate_pixels_mut() {
        // `x < w` and `y < h`, so each quotient is at most 254, and the xor operand is
        // masked to five bits — every value below provably fits in a u8.
        let r = u8::try_from((x * 255) / w).unwrap();
        let g = u8::try_from((y * 255) / h).unwrap();
        let b = u8::try_from(((x + y) * 255) / (w + h)).unwrap()
            ^ u8::try_from((x ^ y) & 0x1f).unwrap();
        *px = image::Rgb([r, g, b]);
    }
    DynamicImage::ImageRgb8(buf)
}

fn main() {
    let width: u32 = std::env::args()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(2400);
    let quality: u8 = 90;

    // Setup (not measured): build the source and encode it to a JPEG buffer,
    // standing in for the original file on disk.
    let src = synthetic_source(SRC_W, SRC_H);
    let mut jpeg = Vec::new();
    src.write_to(&mut Cursor::new(&mut jpeg), image::ImageFormat::Jpeg)
        .expect("encode source jpeg");

    let profiler = dhat::Profiler::builder().build();
    // The measured unit of work: one full transcode (the server's real path).
    let img = image::load_from_memory(&jpeg).expect("decode");
    let resized = resize_for_width(&img, width);
    let webp = convert_to_webp(&resized, quality).expect("encode");
    std::hint::black_box(&webp);

    let stats = dhat::HeapStats::get();
    drop(profiler); // flushes dhat-heap.json
    println!("--- image transcode alloc profile (src {SRC_W}x{SRC_H} -> w{width} q{quality}) ---");
    println!("total allocations: {}", stats.total_blocks);
    println!("total bytes:       {}", stats.total_bytes);
    println!("peak bytes:        {}", stats.max_bytes);
    println!("webp output:       {} bytes", webp.len());
    println!("wrote dhat-heap.json");
}
