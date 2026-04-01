---
title: Benchmark Guide
sidebar_position: 8
sidebar_label: Benchmarks
description: Benchmark suite, dataset lifecycle, R2 auto-download, and common execution patterns.
---

# Benchmark Guide

The TTOON benchmark suite compares the Python, JavaScript, and Rust SDKs across object-path, Arrow-path, and streaming operations.

## Layout and Authority Manifests

- benchmark entry point: `benchmarks/bench.sh`
- Python runner: `benchmarks/python/runner.py`
- full sweep: `benchmarks/scripts/run_all.py`
- benchmark release pin: `benchmarks/manifests/benchmark_release.sh`
- dataset release and SHA256 pins: `benchmarks/manifests/datasets.sh`

## Dataset Preparation Flow

The default dataset root is `benchmarks/datasets/prepared`.

When a benchmark entry point finds that `prepared/<variant>/<size>/meta.json` is missing, it now performs this flow:

1. if `prepared/<variant>/<size>.tar.zst` already exists locally, verify SHA256 and unpack it
2. if the local archive is missing, download it from `DATASET_BASE_URL`
3. verify the downloaded archive against the SHA256 pin in `benchmarks/manifests/datasets.sh`
4. unpack it into `prepared/<variant>/<size>/`

In other words, benchmarks can now fetch missing datasets directly from R2 instead of requiring manual provisioning first.

## Common Commands

### List Datasets

```bash
./benchmarks/bench.sh --list-datasets
```

### List Cases

```bash
./benchmarks/bench.sh --language js --variant js-basic --shape structure --list-cases
```

### Run a Single Benchmark

```bash
./benchmarks/bench.sh --language js --variant js-basic --size 10k --shape structure
./benchmarks/bench.sh --language python --variant js-basic --size 100k --shape tabular --api streaming
./benchmarks/bench.sh --language rust --variant extended --size 100k --case arrow_ttoon_deserialize
```

### Run the Full Sweep and Generate Summary / Report

```bash
uv run --project python python benchmarks/scripts/run_all.py
```

## Language-Specific Notes

### Python

- `benchmarks/bench.sh` automatically ensures the Python extension is a release build
- if you invoke `benchmarks/python/runner.py` directly, it is still best to run:

```bash
cd python
uv run maturin develop --release
```

### JavaScript

- JS benchmarks require `rust/crates/wasm-bridge/pkg`
- if `pkg/` is missing and `wasm-pack` is available, the scripts build it automatically
- if `pkg/` is missing and `wasm-pack` is unavailable, JS benchmarks fail fast

### Rust

- Rust benchmarks use the release build path from the workspace directly

## Result Outputs

`run_all.py` writes the following under `benchmarks/results/<timestamp>/`:

- `raw/*.json`
- `summary.json`
- `summary.csv`
- `report.md`

These outputs embed both `benchmark_release` and `dataset_release` so reports, datasets, and release notes stay aligned.

## Read Existing Datasets Only

Python entry points support:

```bash
uv run --project python python benchmarks/scripts/run_all.py --no-auto-unpack
uv run --project python python benchmarks/python/runner.py --no-auto-unpack
```

This mode reads existing extracted datasets only and does not download or unpack archives.
