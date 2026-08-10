use std::process::Command;

/// Run one CI step. Exits with the step's own status code if it fails, so the first
/// failure stops the run and `cargo ci` exits non-zero.
fn step(args: &[&str]) {
    println!("\n=> cargo {}", args.join(" "));
    let status = Command::new("cargo")
        .args(args)
        .status()
        .unwrap_or_else(|e| panic!("failed to execute `cargo {}`: {e}", args.join(" ")));
    if !status.success() {
        std::process::exit(status.code().unwrap_or(1));
    }
}

fn main() {
    // These mirror .forgejo/workflows/ci.yml step for step. Keep them in sync: the
    // point of `cargo ci` is that passing here means CI will pass too.

    // --- the `rust` job ---
    step(&["fmt", "--all", "--check"]);
    // `ssr` is the server build; the majority of the crate is behind it, so linting
    // without it would skip most of the code. --all-targets also covers the benches
    // and examples.
    step(&[
        "clippy",
        "--features",
        "ssr",
        "--all-targets",
        "--",
        "-D",
        "warnings",
    ]);
    // No --lib: that skips the doctests, and it is what `cargo leptos test` runs for
    // the server half.
    step(&["test", "--features", "ssr"]);

    // --- the `hydrate` job ---
    // The client half of `cargo leptos test`. The two ssr steps above never compile
    // client-only code (anything behind #[cfg(feature = "hydrate")]), so without these
    // a green `cargo ci` could still fail CI — which is exactly how 8 clippy warnings
    // accumulated in src/app.rs unnoticed.
    // --tests so the #[cfg(test)] modules get linted under `hydrate`, not merely
    // compiled by the test step below. Without it, a warning that only fires in the
    // client-side test build lands in `cargo test` output and gates nothing.
    step(&[
        "clippy",
        "--lib",
        "--tests",
        "--no-default-features",
        "--features",
        "hydrate",
        "--",
        "-D",
        "warnings",
    ]);
    step(&[
        "test",
        "--lib",
        "--no-default-features",
        "--features",
        "hydrate",
    ]);

    // Not covered: CI's `wasm` job (cargo leptos build --release, then
    // scripts/perf/wasm_size.sh against wasm-budget.txt). It needs a full release
    // build, which is far too slow for a pre-push check — run it by hand when a change
    // could have moved the bundle size.
    println!("\n=> ✅ CI checks passed!");
}
