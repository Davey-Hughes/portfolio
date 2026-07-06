//! Microbenchmark for the per-gallery mosaic layout solver.
//!
//! Galleries with `use_mosaic = true` build their layout via
//! `generate_mosaic_with_images`: a bin-packing pass that places rectangles and
//! assigns images by aspect ratio. The result is cached (1h TTL), so this runs
//! on a cache miss / gallery change, scaling with the photo count. It uses a
//! thread RNG internally, so this is a **timing-only** bench (comparable across
//! runs because criterion averages many iterations, but not an exact
//! allocation-count reference — use `alloc_profile` for that).
//!
//! Run: `cargo bench --features ssr --bench mosaic`

use criterion::{BenchmarkId, Criterion, Throughput, black_box, criterion_group, criterion_main};
use portfolio::mosaic::{MosaicConfig, calculate_orientation_bias, generate_mosaic_with_images};

/// Deterministic mix of landscape / portrait / square aspect ratios.
fn synthetic_aspects(n: usize) -> Vec<(usize, f64)> {
    const RATIOS: &[f64] = &[1.5, 0.667, 1.0, 1.333, 0.75, 1.777];
    (0..n).map(|i| (i, RATIOS[i % RATIOS.len()])).collect()
}

/// Build the config the desktop request path uses (see
/// `server::generate_mosaic_layout_for_size`).
fn config_for(aspects: &[(usize, f64)]) -> MosaicConfig {
    const CONTAINER_WIDTH: f64 = 1200.0;
    const BASE_HEIGHT: f64 = 600.0;
    const PHOTOS_PER_BASE_HEIGHT: f64 = 3.0;
    let scale = (aspects.len() as f64 / PHOTOS_PER_BASE_HEIGHT).max(2.0);
    MosaicConfig {
        container_width: CONTAINER_WIDTH,
        container_height: BASE_HEIGHT * scale,
        min_cell_dimension: 180.0,
        min_aspect_ratio: 0.4,
        max_aspect_ratio: 3.0,
        orientation_bias: Some(calculate_orientation_bias(aspects)),
    }
}

fn bench_mosaic(c: &mut Criterion) {
    let mut g = c.benchmark_group("mosaic_layout");
    for &n in &[20usize, 50, 100, 200] {
        let aspects = synthetic_aspects(n);
        g.throughput(Throughput::Elements(n as u64));
        g.bench_with_input(BenchmarkId::from_parameter(n), &aspects, |b, aspects| {
            b.iter(|| {
                generate_mosaic_with_images(
                    black_box(aspects.len()),
                    black_box(aspects),
                    config_for(aspects),
                    100,
                )
            });
        });
    }
    g.finish();
}

criterion_group!(mosaic, bench_mosaic);
criterion_main!(mosaic);
