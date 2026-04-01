---
title: Object Path vs Arrow Path
sidebar_position: 5
sidebar_label: Object vs Arrow Path
description: Understanding the two independent processing paths in TTOON.
---

# Object Path vs Arrow Path

TTOON maintains two independent processing paths. Understanding when to use which is key to getting the best performance and developer experience.

## Object Path

The general-purpose path that converts between text and language-native objects.

```text
T-TOON/T-JSON text ──parse──→ IR ──convert──→ Python dict / JS object / Rust Node
Python dict / JS object / Rust Node ──convert──→ IR ──serialize──→ T-TOON/T-JSON text
```

**Characteristics:**
- Works with any data shape (objects, arrays, scalars, nested structures)
- Produces familiar language-native types (`dict`, `object`, `Node`)
- Goes through IR (internal representation) as intermediate step
- Suitable for small to medium datasets, configs, and general-purpose exchange

**APIs:**

| Language | Parse | Serialize T-TOON | Serialize T-JSON |
| :--- | :--- | :--- | :--- |
| Python | `loads()` | `dumps(obj)` | `to_tjson(obj)` |
| JS | `parse()` | `stringify()` | `toTjson()` |
| Rust | `from_ttoon()` | `to_ttoon()` | `to_tjson()` |

## Arrow Path

The high-performance path for tabular data that reads/writes Apache Arrow columnar format directly.

```text
T-TOON/T-JSON text ──direct parse──→ Arrow columnar data
Arrow columnar data ──direct serialize──→ T-TOON/T-JSON text
```

**Characteristics:**
- Only works with tabular data (list of uniform objects with scalar fields)
- T-JSON can build Arrow Table / RecordBatch directly; T-TOON tabular still uses the compatibility path
- Zero-copy where possible; minimal memory allocation
- Preserves native Arrow types (`Decimal128`, `Date32`, `FixedSizeBinary(16)`)
- Significantly faster and more memory-efficient for large datasets

**APIs:**

| Language | Parse | Serialize T-TOON | Serialize T-JSON |
| :--- | :--- | :--- | :--- |
| Python | `read_arrow()` | `dumps(table/df)` | `stringify_arrow_tjson()` |
| JS | `readArrow()` | `stringifyArrow()` | `stringifyArrowTjson()` |
| Rust | `read_arrow()` | `arrow_to_ttoon()` | `arrow_to_tjson()` |

## When to Use Which

| Scenario | Path | Reason |
| :--- | :--- | :--- |
| Config files | Object | Arbitrary nesting, small size |
| API payloads | Object | General-purpose, any shape |
| Database table exports | Arrow | Tabular, potentially large |
| Polars/Pandas pipelines | Arrow | Already in columnar format |
| Streaming large datasets | Arrow (streaming) | Memory-efficient row-by-row |
| Cross-language object exchange | Object | Familiar native types |
| Analytics pipelines | Arrow | Native Arrow ecosystem |

## Streaming Variants

Both paths have streaming variants for row-by-row processing:

| Path | Streaming Reader | Streaming Writer |
| :--- | :--- | :--- |
| Object | `StreamReader` / `streamRead()` | `StreamWriter` / `streamWriter()` |
| Arrow | `ArrowStreamReader` / `streamReadArrow()` | `ArrowStreamWriter` / `streamWriterArrow()` |

Plus T-JSON variants of each (`TjsonStreamReader`, `TjsonArrowStreamWriter`, etc.).

See the [Streaming Guide](../guides/streaming.md) for details.

## Design Rationale

The two paths are deliberately kept independent rather than forcing all data through a single pipeline. This is because:

1. **Performance**: Arrow columnar reads/writes avoid the overhead of row-by-row IR conversion
2. **Type fidelity**: Arrow native types (`Decimal128`, `FixedSizeBinary(16)`) are preserved without lossy conversion
3. **Memory efficiency**: Large datasets never materialize as language-native object trees
4. **Code duplication is acceptable**: The small amount of shared logic between paths is an intentional trade-off for performance
