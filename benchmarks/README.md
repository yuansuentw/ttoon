# Cross-language Benchmark Suite

> last update: 2026-04-01 12:10 +0800

This directory contains the benchmark datasets, runners, manifests, scripts, and reports used to compare TTOON across Python, JavaScript, and Rust.

## Layout

```text
benchmarks/
|- datasets/
|- js/
|- manifests/
|- python/
|- results/
|- rust/
|- scripts/
`- specs/
```

## Common Commands

```bash
# Shell entry point
./benchmarks/bench.sh --list-datasets
./benchmarks/bench.sh --language js --variant js-basic --size 10k --shape structure

# Filter by API mode with --api=batch|streaming|all
./benchmarks/bench.sh --language python --variant js-basic --size 100k --shape tabular --api batch
./benchmarks/bench.sh --language python --variant js-basic --size 100k --shape tabular --api streaming
./benchmarks/bench.sh --language rust --variant js-basic --size 100k --shape tabular --api streaming

# Build the Python extension in release mode before running the Python runner directly
cd python && uv run maturin develop --release

# Generate prepared datasets
uv run --project python python benchmarks/scripts/prepare_datasets.py

# Verify that the frozen bundles are reproducible
uv run --project python python benchmarks/scripts/prepare_datasets.py --validate-only --verify-reproducibility

# Unpack existing .tar.zst bundles
uv run --project python python benchmarks/scripts/unpack_datasets.py

# Run every available runner and generate summary/report outputs
uv run --project python python benchmarks/scripts/run_all.py

# Run the Python runner only
uv run --project python python benchmarks/python/runner.py

# Rebuild summaries from existing raw results
uv run --project python python benchmarks/scripts/summarize.py
```

## Notes

- `benchmarks/results/` is not versioned. The directory only keeps `.gitkeep`.
- `benchmarks/manifests/benchmark_release.sh` and `benchmarks/manifests/datasets.sh` define the release identifiers embedded in raw results, `summary.json`, `summary.csv`, and `report.md`.
- Dataset bundles such as `10k`, `100k`, and `1m` are distributed as external `.tar.zst` artifacts. This repository keeps manifests, scripts, and metadata rather than large extracted datasets.
- `benchmarks/bench.sh` builds the Python extension in release mode before running Python benchmarks so that debug builds do not skew results.
- If you run `benchmarks/python/runner.py` directly, build the extension in release mode first. The runner fails fast when it detects a non-release `_core`.
- Streaming benchmarks measure runtime only. Correctness validation belongs in the test suite, not in the benchmark loop.
- The JavaScript benchmark environment is split into two layers:
  - `benchmarks/js/` keeps only the base runtime tools (`tsx`, `apache-arrow`).
  - The benchmarked package `@ttoon/shared` is installed on demand from the sibling source tree (`../../js/shared`) with `--no-save --no-package-lock`, so the benchmark manifest does not permanently pin the formal package into `benchmarks/js/package.json`.
- When the JS runner needs `@ttoon/shared`, it also requires `rust/crates/wasm-bridge/pkg`.
  - If `pkg/` is missing and `wasm-pack` is available, the runner will build it automatically.
  - If `pkg/` is missing and `wasm-pack` is not installed, JS benchmarks fail fast with an environment error.
- The `toon_*` comparison cases install `@toon-format/toon` from npm on demand with `--no-save --no-package-lock`.
  - The repository no longer vendors a local `js/toon/` package.
  - If you do not run `toon_serialize` / `toon_deserialize`, the official TOON package is not installed.
- `benchmarks/bench.sh`, `run_all.py`, and the Python runner now auto-fetch missing dataset archives from `DATASET_BASE_URL`, verify SHA256 against `benchmarks/manifests/datasets.sh`, then unpack them locally.
- Use `--no-auto-unpack` if you want Python entry points to read only existing extracted directories without downloading or unpacking anything.
- Use `--dataset-root` if your prepared datasets live outside the default location.
