//! Microbenchmarks for the per-photo metadata string work.
//!
//! On discovery every photo has its EXIF `UserComment`/`ImageDescription`
//! parsed for film stock (two export formats: the current pipe-delimited one
//! and the legacy LensTagger one) and its filename turned into a title + slug.
//! These are pure string transforms run once per file when a gallery is
//! (re)loaded; this bench measures them so a parsing change can be verified not
//! to regress bulk-load time.
//!
//! Run: `cargo bench --features ssr --bench metadata`

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use portfolio::gallery::{parse_film_stock_from_comment, strip_leading_number_and_dash};

// Current pipe-delimited export: film stock is the `Film:` value, gauge dropped.
const PIPE_COMMENT: &str =
    "Camera: Nikon F3 | Lens: Nikkor 50mm f/1.4 | Film: Kodak Portra 400, 35mm | \
     Dev: Cinestill CS41 | Scan: Noritsu HS-1800";

// Legacy LensTagger export: detected by the `LensTaggerVer` marker, assembled
// from newline-delimited Film Make/Type and the -ISO override.
const LENSTAGGER_COMMENT: &str = "LensTaggerVer=0.9.0\n\
     Lens: Nikkor 50mm\n\
     Film Make: Kodak\n\
     Film Type: Gold\n\
     -ISO=200";

fn bench_film_stock(c: &mut Criterion) {
    let mut g = c.benchmark_group("film_stock");
    g.bench_function("pipe_delimited", |b| {
        b.iter(|| parse_film_stock_from_comment(black_box(PIPE_COMMENT)));
    });
    g.bench_function("lenstagger", |b| {
        b.iter(|| parse_film_stock_from_comment(black_box(LENSTAGGER_COMMENT)));
    });
    g.finish();
}

fn bench_title_derivation(c: &mut Criterion) {
    // The real derivation: strip a leading `N - ` / `N.N - ` ordering prefix,
    // drop the extension, then turn '-'/'_' into spaces for the title.
    let filename = "12 - golden_gate-at_sunset.jpg";
    c.bench_function("title_from_filename", |b| {
        b.iter(|| {
            strip_leading_number_and_dash(black_box(filename))
                .trim_end_matches(".jpg")
                .replace(['-', '_'], " ")
        });
    });
}

criterion_group!(metadata, bench_film_stock, bench_title_derivation);
criterion_main!(metadata);
