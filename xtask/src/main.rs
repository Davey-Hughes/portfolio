use std::process::Command;

const USAGE: &str = "usage: cargo ci [--release]";

/// Where `--release` writes the e2e fixtures, as CI and the recipe in
/// docs/development.md do. The one folder the runner ever deletes. The delete names
/// this literal and nothing else: no variable, joined or computed path that could come
/// out empty and turn it into something wider.
const E2E_FIXTURES: &str = "target/e2e-fixtures";

/// Run one CI step. Exits with the step's own status code if it fails, so the first
/// failure stops the run and `cargo ci` exits non-zero.
fn run(program: &str, args: &[&str], env: &[(&str, &str)]) {
    // The command as you'd type it, environment first.
    let shown = env
        .iter()
        .map(|(k, v)| format!("{k}={v}"))
        .chain([program.to_owned()])
        .chain(args.iter().map(|a| (*a).to_owned()))
        .collect::<Vec<_>>()
        .join(" ");
    println!("\n=> {shown}");
    let status = Command::new(program)
        .args(args)
        .envs(env.iter().copied())
        .status()
        .unwrap_or_else(|e| panic!("failed to execute `{shown}`: {e}"));
    if !status.success() {
        std::process::exit(status.code().unwrap_or(1));
    }
}

/// A `cargo` step with no extra environment, which is most of them.
fn step(args: &[&str]) {
    run("cargo", args, &[]);
}

fn main() {
    // Parse up front, so a typo (`--releas`) fails at once rather than quietly running
    // less than was asked for.
    let args: Vec<String> = std::env::args().skip(1).collect();
    let release = match args.as_slice() {
        [] => false,
        [flag] if flag == "--release" => true,
        _ => {
            eprintln!("error: unexpected arguments: {}\n{USAGE}", args.join(" "));
            std::process::exit(2);
        }
    };

    // The relative paths below (scripts/, target/) assume the repository root as the
    // working directory. The alias guarantees it: its `--manifest-path xtask/Cargo.toml`
    // only resolves from there.

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
    // the server half. That command also adds `--cfg erase_components`, as every
    // cargo-leptos debug build does. CI and the release build leave it out, and so does
    // this, so the tests see the HTML the site sends.
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

    // --- the `release-checks` job, behind --release ---
    // It needs a full release build, which is too slow to run on every check. CI gates
    // every pull request on this job anyway, so locally it is opt-in: run it when a
    // change could affect the release build, the fixtures or the bundle size.
    //
    // Not covered even then: the job's Playwright suite, which needs a server running
    // against the fixtures. The recipe is under Testing in docs/development.md.
    if release {
        step(&["leptos", "build", "--release"]);
        // CI generates the fixtures because this is the step that fails when an example
        // stops building against the real feature set, which the tests above don't
        // exercise. Into target/, as CI does: the default would write fixture photos into
        // public/images.
        //
        // Cleared first, as CI and the recipe do. gen_fixtures writes over what is there
        // but never removes anything, so a renamed fixture (`bird.jpg` became `bird.JPG`)
        // would sit beside the old one and make two files of one photo.
        println!("\n=> rm -rf {E2E_FIXTURES}");
        match std::fs::remove_dir_all(E2E_FIXTURES) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => {
                eprintln!("error: couldn't remove {E2E_FIXTURES}: {e}");
                std::process::exit(1);
            }
        }
        let fixture_images = format!("{E2E_FIXTURES}/images");
        run(
            "cargo",
            &[
                "run",
                "--release",
                "--example",
                "gen_fixtures",
                "--features",
                "ssr",
            ],
            &[("FIXTURE_IMAGES_DIR", &fixture_images)],
        );
        // Exits 2 when the gzipped bundle is over scripts/perf/wasm-budget.txt.
        run("scripts/perf/wasm_size.sh", &[], &[]);
        println!("\n=> ✅ CI checks passed, apart from the Playwright suite!");
    } else {
        println!("\n=> ✅ CI checks passed! (`cargo ci --release` adds the release build)");
    }
}
