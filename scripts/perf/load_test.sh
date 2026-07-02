#!/usr/bin/env bash
# SSR latency load test. Start the server first (in another terminal):
#   cargo leptos serve --release
# Usage: scripts/perf/load_test.sh [base_url]   (default http://127.0.0.1:4000)
#
# By default it hits the always-present pages (/, /about, /contact). To also
# load-test a gallery and the CPU-heavy compressed-image endpoint (the real hot
# path), point it at content you have:
#   GALLERY=film IMG_PATH=film/some-photo scripts/perf/load_test.sh
set -euo pipefail
BASE="${1:-${BASE_URL:-http://127.0.0.1:4000}}"

routes=("/" "/about" "/contact")
[ -n "${GALLERY:-}" ] && routes+=("/gallery/$GALLERY")
# The compressed endpoint transcodes on a cache miss; the first hit warms the
# cache, so this measures the cache-hit serve path unless the cache is cleared.
[ -n "${IMG_PATH:-}" ] && routes+=("/images/compressed/${IMG_PATH}?width=2400&quality=90")

if ! curl -fsS -o /dev/null "$BASE/" 2>/dev/null; then
  echo "Server not reachable at $BASE — start it: cargo leptos serve --release" >&2
  exit 1
fi

run() {
  local tool="$1"; shift
  for r in "${routes[@]}"; do
    echo "== $tool $BASE$r =="
    "$tool" "$@" "$BASE$r"
  done
}

if command -v oha >/dev/null 2>&1; then
  run oha -n 500 -c 20 --no-tui
elif command -v hey >/dev/null 2>&1; then
  run hey -n 500 -c 20
else
  echo "Need a load tester. Install one:" >&2
  echo "  cargo install oha                              # preferred" >&2
  echo "  go install github.com/rakyll/hey@latest" >&2
  exit 1
fi
