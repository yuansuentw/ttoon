#!/usr/bin/env bash

# shellcheck shell=bash
#
# Benchmark release manifest.
# This file is the authoritative release pin for the benchmark suite.

BENCHMARK_MANIFEST_VERSION="1"
BENCHMARK_RELEASE="1.0"
BENCHMARK_DATASET_RELEASE="1"

BENCHMARK_LANGUAGES=("python" "js" "rust")
BENCHMARK_VARIANTS=("js-basic" "extended")
BENCHMARK_SIZES=("10k" "100k" "1m")

BENCHMARK_NOTES_REF="docs/dev/005-20260309-cross-language-benchmark-suite/20260311-1310-benchmark-methodology-notes.md"
BENCHMARK_STORAGE_PROVIDER="local"
BENCHMARK_DEFAULT_DATASET_ROOT="benchmarks/datasets/prepared"
