# TTOON Documentation Index

## Getting Started

- [Introduction](intro.md) — technical overview of formats, SDKs, and processing paths
- [Installation](getting-started/installation.md) — package install commands and environment notes
- [Quick Start](getting-started/quick-start.md) — first parse / stringify round trip
- [Format Overview](getting-started/format-overview.md) — T-TOON, T-JSON, typed values, and tabular syntax
- Or visit our official website [ttoon.dev](https://ttoon.dev/).

## Concepts

- [Why TTOON?](concepts/why-ttoon.md) — format design goals and expected use cases
- [T-TOON vs T-JSON](concepts/ttoon-vs-tjson.md) — structural and behavioral differences
- [Parse Modes](concepts/parse-modes.md) — `compat` and `strict`
- [Object Path vs Arrow Path](concepts/object-path-vs-arrow-path.md) — native object flow vs columnar flow
- [Performance](concepts/performance.md) — benchmark framing and performance model

## Guides

- [Python](guides/python.md) — Python object path, Arrow path, and common patterns
- [JavaScript / TypeScript](guides/js-ts.md) — JS APIs, packages, and environment notes
- [Rust](guides/rust.md) — crate-level usage and options
- [Transcode](guides/transcode.md) — direct `T-TOON` / `T-JSON` conversion
- [Arrow and Polars](guides/arrow-and-polars.md) — tabular and columnar workflows
- [Streaming](guides/streaming.md) — row-by-row readers and writers
- [JS Codecs and Int64](guides/js-codecs-and-int64.md) — JS codecs, `BigInt`, and precision handling
- [Benchmarks](guides/benchmarks.md) — suite layout, datasets, and commands
- [Contributors](guides/contributors.md) — public-doc sync and release prep

## Reference

- [Typed Value Reference](reference/typed-value-reference.md) — the 12 typed types and encoding rules
- [API Matrix](reference/api-matrix.md) — cross-language API coverage
- [T-TOON Batch API](reference/ttoon-batch-api.md) — non-streaming APIs centered on T-TOON text
- [T-JSON Batch API](reference/tjson-batch-api.md) — non-streaming APIs centered on T-JSON text
- [Stream API](reference/stream-api.md) — stream readers, writers, and schema types
- [Format Detection](reference/format-detection.md) — `ttoon`, `tjson`, and `typed_unit` detection rules
- [Behaviors and Limitations](reference/behaviors-and-limitations.md) — edge cases and explicit constraints
- [Troubleshooting](reference/troubleshooting.md) — common failure modes and diagnosis
