---
title: Introduction
sidebar_position: 1
sidebar_label: Introduction
description: Technical overview of TTOON formats, SDKs, and processing paths.
---

# TTOON

TTOON is a typed plain text interchange system with two complementary syntaxes under one project:

- **T-TOON**: indentation-based structure extended from TOON v3.0
- **T-JSON**: JSON structure with the same typed value system at the leaf level

They are not separate products. They are two serialization syntaxes backed by the same typed model and the same Rust core implementation.

## Format Model

| Syntax | Structure | Typical use |
| :--- | :--- | :--- |
| `T-TOON` | indentation-based, supports tabular `[N]{fields}:` blocks | readable config-like data and tabular datasets |
| `T-JSON` | JSON-compatible structure with typed leaf values | integration points that need JSON-style containers |

Both syntaxes support the same typed values. The current typed set covers:

- `null`, `bool`, `int`, `float`, `decimal`, `string`
- `date`, `time`, `datetime`
- `uuid`, `hex`, `b64`

See [Typed Value Reference](reference/typed-value-reference.md) for the exact encoding rules and examples.

## SDKs and Shared Core

| Language | Package | Architecture |
| :--- | :--- | :--- |
| Python | `ttoon` | Rust core via PyO3 bridge |
| JavaScript / TypeScript | `@ttoon/shared` | Rust core via WASM bridge |
| JavaScript / Node.js | `@ttoon/node` | Re-exports `@ttoon/shared` |
| JavaScript / Web | `@ttoon/web` | Re-exports `@ttoon/shared` |
| Rust | `ttoon-core` | Core engine (canonical implementation) |

Python, JavaScript, and Rust all route to the same Rust core for parsing and serialization semantics. The public API surface is documented in [API Matrix](reference/api-matrix.md).

## Processing Paths

TTOON exposes two execution paths across the SDKs:

- **object path**: parse into language-native objects and serialize from native objects
- **Arrow path**: parse and serialize columnar data directly, without materializing row objects first

The API surface is also split by I/O style:

- **batch APIs**: parse or stringify whole documents or tables
- **stream APIs**: read or write row-by-row with schema-driven contracts

See:

- [Object Path vs Arrow Path](concepts/object-path-vs-arrow-path.md)
- [T-TOON Batch API](reference/ttoon-batch-api.md)
- [T-JSON Batch API](reference/tjson-batch-api.md)
- [Stream API](reference/stream-api.md)

## Minimal Example

```ttoon
user:
  id: uuid(550e8400-e29b-41d4-a716-446655440000)
  name: "Alice"
  joined: 2026-03-08
  balance: 123.45m
tags: [2]:
  - "alpha"
  - "beta"
```

This document round-trips with native typed results instead of collapsing everything to strings:

- `uuid(...)` stays UUID-like instead of a plain string marker
- `2026-03-08` stays a date value
- `123.45m` stays decimal
- tabular and Arrow paths can preserve columnar semantics for datasets

## Read Next

- **[Installation](getting-started/installation.md)** — Set up TTOON in your project
- **[Quick Start](getting-started/quick-start.md)** — first end-to-end examples per language
- **[Format Overview](getting-started/format-overview.md)** — exact syntax and typed value rules
- **[Typed Value Reference](reference/typed-value-reference.md)** — per-type encoding details
- **[API Matrix](reference/api-matrix.md)** — cross-language API comparison
