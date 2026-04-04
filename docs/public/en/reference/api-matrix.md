---
title: API Matrix
sidebar_position: 1
sidebar_label: API Matrix
description: Cross-language API comparison for Rust, JavaScript, and Python — 18/18 parity.
---

# API Matrix

All three TTOON SDKs provide a fully aligned API surface: **18/18** across Rust, JavaScript, and Python.

## Grouped Reference Pages

The detailed API reference is organized by workload instead of by language:

- **[T-TOON Batch API](./ttoon-batch-api.md)** — batch read / write / transcode APIs centered on T-TOON text
- **[T-JSON Batch API](./tjson-batch-api.md)** — batch read / write / transcode APIs centered on T-JSON text
- **[Stream API](./stream-api.md)** — streaming readers and writers for both formats

## Batch Deserialization

| Capability | Rust | JavaScript | Python |
| :--- | :--- | :--- | :--- |
| Text → Object/IR | `from_ttoon(text)` | `parse(text)` | `loads(text)` |
| Text → Arrow | `read_arrow(text)` | `readArrow(text)` | `read_arrow(text)` |

All batch deserialization APIs auto-detect the input format (T-TOON / T-JSON / typed unit).

## Batch Serialization

| Capability | Rust | JavaScript | Python |
| :--- | :--- | :--- | :--- |
| Object → T-TOON | `to_ttoon(node)` | `stringify(value)` | `dumps(obj)` |
| Object → T-JSON | `to_tjson(node)` | `toTjson(value)` | `to_tjson(obj)` |
| Arrow → T-TOON | `arrow_to_ttoon(table)` | `stringifyArrow(table)` | `dumps(df/table)` |
| Arrow → T-JSON | `arrow_to_tjson(table)` | `stringifyArrowTjson(table)` | `stringify_arrow_tjson(table)` |

Python `dumps()` auto-detects Polars DataFrame and PyArrow Table/RecordBatch inputs, routing them to the Arrow path internally.

## Streaming Deserialization

| Capability | Rust | JavaScript | Python |
| :--- | :--- | :--- | :--- |
| T-TOON → Object | `StreamReader` | `streamRead(source, opts)` | `stream_read(source, schema)` |
| T-JSON → Object | `TjsonStreamReader` | `streamReadTjson(source, opts)` | `stream_read_tjson(source, schema)` |
| T-TOON → Arrow | `ArrowStreamReader` | `streamReadArrow(source, opts)` | `stream_read_arrow(source, schema)` |
| T-JSON → Arrow | `TjsonArrowStreamReader` | `streamReadArrowTjson(source, opts)` | `stream_read_arrow_tjson(source, schema)` |

All streaming readers require a `StreamSchema` to define field names and types.

## Streaming Serialization

| Capability | Rust | JavaScript | Python |
| :--- | :--- | :--- | :--- |
| Object → T-TOON | `StreamWriter` | `streamWriter(sink, opts)` | `stream_writer(sink, schema)` |
| Object → T-JSON | `TjsonStreamWriter` | `streamWriterTjson(sink, opts)` | `stream_writer_tjson(sink, schema)` |
| Arrow → T-TOON | `ArrowStreamWriter` | `streamWriterArrow(sink, opts)` | `stream_writer_arrow(sink, schema)` |
| Arrow → T-JSON | `TjsonArrowStreamWriter` | `streamWriterArrowTjson(sink, opts)` | `stream_writer_arrow_tjson(sink, schema)` |

All streaming writers require a `StreamSchema`.

## Direct Transcode

| Capability | Rust | JavaScript | Python |
| :--- | :--- | :--- | :--- |
| T-JSON → T-TOON | `tjson_to_ttoon(text)` | `tjsonToTtoon(text)` | `tjson_to_ttoon(text)` |
| T-TOON → T-JSON | `ttoon_to_tjson(text, mode)` | `ttoonToTjson(text)` | `ttoon_to_tjson(text)` |

Transcode passes through Rust IR only — no language-native objects are materialized. All typed semantics (decimal, uuid, etc.) are fully preserved.

## Utilities

| Capability | Rust | JavaScript | Python |
| :--- | :--- | :--- | :--- |
| Format detection | `detect_format(text)` | `detectFormat(text)` | `detect_format(text)` |
| Codec registration | — (native types) | `use(codecs)` | `use(codecs)` |
| Schema definition | `StreamSchema` | `StreamSchema` | `StreamSchema` |

## Coverage Statistics

| Dimension | Rust | JavaScript | Python |
| :--- | :--- | :--- | :--- |
| Batch (deser + ser) | 6/6 | 6/6 | 6/6 |
| Streaming (deser + ser) | 8/8 | 8/8 | 8/8 |
| Transcode | 2/2 | 2/2 | 2/2 |
| Utilities | 2/2 | 2/2 | 2/2 |
| **Total** | **18/18** | **18/18** | **18/18** |

Codec registration is excluded from the 18/18 parity count because it is JS/Python-only; the matrix counts only APIs that exist across all three SDKs.

## Architecture Notes

### Rust as Canonical Engine

Rust `ttoon-core` is the canonical implementation. Both the Python and JavaScript SDKs delegate to the Rust core:

- **Python** — via PyO3 native extension (compiled into the wheel)
- **JavaScript** — via WASM bridge (bundled in the npm package)

This ensures identical parsing and serialization behavior across all platforms.

### Streaming Format Conventions

- **T-TOON streaming** uses `[*]{fields}:` as the unbounded tabular header (in contrast to `[N]{fields}:` which declares a fixed row count).
- **T-JSON streaming** uses a top-level array of objects with schema-known scalar values.

### Arrow Path Optimization

The Rust core includes a direct Arrow path for T-JSON input (`tjson_arrow::read_arrow_tjson_direct`) that skips the Token/Node intermediate layer, significantly reducing memory usage for large datasets. This optimization benefits all SDKs through the shared core.
