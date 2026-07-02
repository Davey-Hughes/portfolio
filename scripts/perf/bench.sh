#!/usr/bin/env bash
# Run the criterion benchmarks and point at the HTML report.
# Pass through any extra args, e.g.:  scripts/perf/bench.sh --bench image_pipeline
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$ROOT"
cargo bench --features ssr "$@"
echo
echo "Criterion HTML report: $ROOT/target/criterion/report/index.html"
