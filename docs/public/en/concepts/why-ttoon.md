---
title: Why TTOON?
sidebar_position: 1
sidebar_label: Why TTOON?
description: Motivation, positioning, and use cases for TTOON as a typed plain text interchange format.
---

# Why TTOON?

## The Problem

Exchanging structured data between systems typically forces a trade-off:

| Format | Human Readable | Typed | High Performance |
| :--- | :--- | :--- | :--- |
| JSON | Moderate | No — everything is string/number/bool/null | No |
| CSV | Yes | No — all cells are strings | No |
| YAML | Yes | Limited — implicit type coercion | No |
| Protocol Buffers | No (binary) | Yes | Yes |
| Apache Arrow IPC | No (binary) | Yes | Yes |
| Parquet | No (binary) | Yes | Yes |

JSON is ubiquitous but cannot distinguish `decimal` from `float`, has no `date`/`time`/`uuid`/`binary` types, and loses precision on large integers. CSV is flat and entirely untyped. YAML attempts implicit coercion (the infamous `"no"` → `false` problem). Binary formats solve the type problem but sacrifice human readability entirely.

## What TTOON Provides

TTOON occupies the gap: **typed, human-readable, plain text, and high-performance**.

- **12 explicit typed encodings** — `null`, `bool`, `int`, `float`, `decimal`, `string`, `date`, `time`, `datetime`, `uuid`, `hex`, `b64` — all unambiguous, no implicit coercion.
- **Two complementary syntaxes** — T-TOON for readability and tabular data; T-JSON for JSON ecosystem compatibility.
- **Cross-language fidelity** — Python `Decimal` ↔ Rust `Decimal128` ↔ JS `string`/`Decimal.js` all pass through the same `123.45m` encoding, preserving precision and intent.
- **Arrow integration with a direct fast path where available** — T-JSON tabular reads use a dedicated Arrow path, while T-TOON still interoperates with Arrow through the shared IR path.
- **Streaming** — Row-by-row readers and writers for both formats, suitable for large datasets and real-time pipelines.

## Use Cases

### Cross-Language Data Exchange

Python data science pipeline → TTOON text → Rust processing engine → TTOON text → JS dashboard. Types survive every hop.

### Database Exports & Auditing

Export database tables as typed plain text for human review, version control diffs, and compliance auditing. Unlike CSV, a `decimal(10,2)` column stays decimal, not a truncated float.

### Arrow / Polars Analytics Pipelines

Inject or extract typed plain text at any point in an Arrow-based pipeline. T-JSON tabular input has a dedicated Arrow read path; T-TOON tabular input still works, but currently parses through the shared IR compatibility route before Arrow conversion.

### Structured Logging

Log entries that are simultaneously human-readable and machine-parseable with preserved type semantics.

### LLM / AI Data Pipelines

Type-safe intermediary between LLM outputs and downstream processing — prevents the silent type coercion bugs that plague JSON-based LLM pipelines.

## Design Philosophy

1. **Explicit over implicit** — No type coercion. `123.45m` is always decimal, `123.45` is always float.
2. **Rust-canonical, cross-language aligned** — One Rust core engine powers all SDKs, ensuring identical behavior.
3. **Two paths, independent** — Object path for general use; Arrow path for columnar performance. Neither forces the other.
4. **Parse is validation** — No separate `validate()` step. If it parses, it is valid.
5. **Format routes don't fallback** — Once the parser determines T-TOON or T-JSON, it commits. No silent retry.
