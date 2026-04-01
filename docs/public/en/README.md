# TTOON


TTOON is a **typed plain text** data exchange format engineered for modern data workflows. It provides two complementary syntaxes — T-TOON (indentation-based) and T-JSON (bracket-based) — with full SDK support for Python, JavaScript/TypeScript, and Rust.

## Documentation

### Getting Started

- [Introduction](intro.md) — What TTOON is and why it exists
- [Installation](getting-started/installation.md) — Set up TTOON in your project
- [Quick Start](getting-started/quick-start.md) — Your first round trip
- [Format Overview](getting-started/format-overview.md) — Understand the two syntaxes

### Concepts

- [Why TTOON?](concepts/why-ttoon.md) — Motivation, positioning, and use cases
- [T-TOON vs T-JSON](concepts/ttoon-vs-tjson.md) — Detailed syntax comparison
- [Typed Values](concepts/typed-values.md) — The 12 typed value encodings
- [Parse Modes](concepts/parse-modes.md) — `compat` vs `strict` mode
- [Object Path vs Arrow Path](concepts/object-path-vs-arrow-path.md) — Two processing paths

### Guides

- [Contributors](guides/contributors.md) — doc sources, sync flow, and release prep
- [Benchmarks](guides/benchmarks.md) — benchmark suite, datasets, and common commands
- [Python](guides/python.md) — Complete Python usage guide
- [JavaScript / TypeScript](guides/js-ts.md) — Complete JS/TS usage guide
- [Rust](guides/rust.md) — Complete Rust usage guide
- [Transcode](guides/transcode.md) — Direct T-JSON ↔ T-TOON conversion
- [Arrow & Polars](guides/arrow-and-polars.md) — High-performance tabular paths
- [Streaming](guides/streaming.md) — Row-by-row readers and writers
- [JS Codecs & Int64](guides/js-codecs-and-int64.md) — Custom type mapping in JS

### Reference

- [API Matrix](reference/api-matrix.md) — Cross-language API comparison (18/18 parity)
- [Python API](reference/python-api.md) — Complete Python API reference
- [JS API](reference/js-api.md) — Complete JavaScript/TypeScript API reference
- [Rust API](reference/rust-api.md) — Complete Rust API reference
- [Type Mapping](reference/type-mapping.md) — Cross-language type conversion table
- [Stream Schema](reference/stream-schema.md) — Schema definitions for streaming
- [Format Detection](reference/format-detection.md) — How format auto-detection works
- [Behaviors & Limitations](reference/behaviors-and-limitations.md) — Edge cases and constraints
- [Troubleshooting](reference/troubleshooting.md) — Common errors and fixes

### Contributors

- [Contributors Guide](guides/contributors.md) — maintainer and doc contributor entry point
- [Benchmark Guide](guides/benchmarks.md) — benchmark suite, dataset auto-download, and result outputs
