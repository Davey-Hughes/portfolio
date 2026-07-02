# Performance tooling

Scripts and benchmarks for measuring this server's performance. The fact that
shapes all of this: **the site serves originals only through the on-the-fly
transcode endpoint** (`/images/compressed/<path>?width=&quality=`), which
decodes the source, resizes it (Lanczos3), and encodes lossy WebP, caching the
result on disk. So the two costs that matter are (1) that per-image transcode on
a cache miss, and (2) the per-request view work when a gallery is (re)loaded —
EXIF/film-stock parsing and mosaic layout. These tools measure exactly those.

**Note on reproducibility.** Benches and the allocation profile use deterministic
in-memory inputs (a fixed synthetic source image, fixed comment strings, a fixed
aspect-ratio mix) so numbers compare across runs and machines. The mosaic solver
uses a thread RNG internally, so its bench is timing-only — use `alloc_profile`
for a stable allocation number.

## Benchmarks (criterion)

```sh
scripts/perf/bench.sh                                  # all benches, then prints the report path
cargo bench --features ssr                             # equivalent
cargo bench --features ssr --bench image_pipeline      # just the transcode pipeline
cargo bench --features ssr --bench metadata            # just EXIF/title string work
cargo bench --features ssr --bench mosaic              # just the mosaic solver
```

- `image_pipeline` — the per-request hot path: `decode` (24MP JPEG), `resize` to
  each preset width (2400/4000), `encode_webp` at q90, and the full `transcode`
  (decode → resize → encode) per preset, with throughput reported per source
  megapixel.
- `metadata` — `film_stock` parsing for both export formats (pipe-delimited and
  legacy LensTagger) and `title_from_filename` (prefix strip + `-`/`_` → space).
- `mosaic` — `mosaic_layout` scaled over 20/50/100/200 photos (timing-only).

Criterion compares against the previous run automatically and reports regressions.

## Allocation profile (dhat)

```sh
cargo run --release --example alloc_profile --features ssr        # width 2400
cargo run --release --example alloc_profile --features ssr 4000   # width 4000
```

Installs dhat as the global allocator, runs one decode → resize → WebP encode,
and prints total allocations / bytes / peak bytes plus the WebP output size. Also
writes `dhat-heap.json` (load it at
https://nnethercote.github.io/dh_view/dh_view.html). Use this to verify
allocation-reduction work — the numbers should drop. As a baseline, one
6000×4000 → 2400px q90 transcode is ~18 allocations with a ~237 MB peak (the
decoded + resized pixel buffers dominate).

## WASM bundle size

```sh
cargo leptos build --release      # required first — only this emits the optimized bundle
scripts/perf/wasm_size.sh         # raw + gzip + brotli vs budget; exits 2 if over
```

The budget lives in `wasm-budget.txt` (`gzip_kb=<N>`). Raise it deliberately when
a feature genuinely grows the bundle. Install `twiggy` (`cargo install twiggy`)
for a per-item code-size breakdown.

## SSR latency under load

```sh
cargo leptos serve --release                  # in one terminal
scripts/perf/load_test.sh                      # in another (default :4000)
scripts/perf/load_test.sh http://host:port     # custom base URL
GALLERY=film IMG_PATH=film/some-photo scripts/perf/load_test.sh   # also hit a gallery + image
```

Reports p50/p95/p99 over `/`, `/about`, `/contact` by default; set `GALLERY` and
`IMG_PATH` to also exercise a gallery page and the compressed-image endpoint.
Needs `oha` (`cargo install oha`, preferred) or `hey`.

## CPU profiling

```sh
scripts/perf/flamegraph.sh                 # profiles the image_pipeline bench
scripts/perf/flamegraph.sh mosaic          # profile a different bench
```

Needs `samply` (`cargo install samply`, preferred — no root) or `flamegraph`
(`cargo install flamegraph`, needs `perf`).
