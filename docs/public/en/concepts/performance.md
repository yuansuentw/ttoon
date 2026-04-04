---
title: Performance Characteristics
sidebar_position: 6
sidebar_label: Performance
description: Processing paths, memory profiles, synchronous API design, and practical data-size guidance.
---

# Performance Characteristics

This page summarises the runtime behaviour of the three processing modes — **object**, **Arrow**, and **streaming** — and provides practical guidance on choosing the right one for your data size.

## Object Path Overhead

The object path converts between text and language-native objects through an IR (internal representation) layer:

```text
text → parse → IR → convert → dict / object
dict / object → convert → IR → serialize → text
```

Every value crosses the FFI boundary individually. In Python this means pyo3 must attempt a sequence of type checks (datetime → date → time → uuid → decimal → bool → int → float → string) for each leaf value. JavaScript pays a similar per-value cost through the WASM bridge.

Serialize is the more expensive direction — each value must be inspected, tagged, and converted before the Rust core can format it. Relative to the Rust core baseline, Python object serialize is roughly **12–28 × slower** and JavaScript roughly **17–18 ×**, because FFI overhead dominates when processing values one at a time.

For configurations, API payloads, and datasets under roughly **10 K rows**, this overhead is negligible. Beyond that, prefer the Arrow path for tabular data.

## Arrow Columnar Advantage

The Arrow path avoids language-native object trees and keeps data in Arrow columnar form. The strongest no-IR route currently exists for T-JSON → Arrow reads; T-TOON tabular still uses the compatibility path before Arrow conversion:

```text
text → Rust core → Arrow IPC bytes → zero-copy table
Arrow table → IPC bytes → Rust core → text
```

Because the Arrow side avoids per-row language-object conversion, the Arrow path is **7–23 × faster** than the object path for equivalent tabular data. Python and Rust achieve near-identical throughput because the only work that crosses the FFI boundary is a single IPC buffer — the core engine does all the heavy lifting.

Arrow types are preserved natively (`Decimal128`, `Date32`, `Timestamp`, `FixedSizeBinary(16)` for UUID) and never degrade to strings.

## Streaming Memory Profile

Batch mode materialises the entire dataset in memory. Streaming processes data row-by-row (object path) or in record batches (Arrow path), holding only the current chunk.

At scale, streaming cuts peak memory dramatically:

- **100 K rows** — streaming uses roughly **1/5** of the batch peak memory
- **1 M rows** — streaming uses roughly **1/20**, because batch memory grows linearly while streaming stays bounded by chunk size

Streaming deserialize is also **30–60 % faster** than batch because it avoids building a single large allocation. Streaming serialize has roughly the same throughput as batch while cutting peak memory by about **40 %**.

**Rule of thumb:**

| Data size | Recommended mode |
| :--- | :--- |
| < 10 K rows | Batch — object or Arrow, whichever fits your data shape |
| 10 K – 100 K rows | Arrow batch for tabular; object batch for nested structures |
| > 100 K rows | Arrow streaming for tabular data |

## Synchronous API Design

All batch and transcode functions (`parse`, `stringify`, `toTjson`, `tjsonToTtoon`, `ttoonToTjson`) are **synchronous** in every SDK. The core engine performs CPU-intensive text processing — parsing, type inference, serialisation — in a single pass with no I/O waits, so a synchronous call is the natural fit.

The only async APIs are those that require dynamic module loading at the boundary:

| API | Why async |
| :--- | :--- |
| `initWasm()` (JS) | Fetches and instantiates the WASM binary |
| `readArrow()` / `stringifyArrow()` / `stringifyArrowTjson()` (JS) | Dynamic import of `apache-arrow` |
| Streaming readers / writers | Inherently incremental; yield/await per chunk |

If you are building a pipeline around TTOON, you can call `parse` / `stringify` / `toTjson` inside tight loops without worrying about promise overhead or event-loop starvation beyond the execution time of the call itself.

## Summary

| | Object path | Arrow path | Arrow streaming |
| :--- | :--- | :--- | :--- |
| **Data shape** | Any | Tabular only | Tabular only |
| **FFI cost** | Per value | Per IPC buffer | Per batch chunk |
| **Sweet spot** | < 10 K rows, nested | 10 K – 100 K rows | > 100 K rows |
| **Memory profile** | Proportional to data | Proportional to data | Bounded by chunk |
| **Sync / async** | Sync | Sync (JS: async boundary) | Async (incremental) |
