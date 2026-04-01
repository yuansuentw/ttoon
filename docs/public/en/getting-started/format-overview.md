---
title: Format Overview
sidebar_position: 3
sidebar_label: Format Overview
description: T-TOON and T-JSON syntax overview, typed value system, and escape rules.
---

# Format Overview

TTOON provides two syntaxes — T-TOON and T-JSON — that share the same typed value layer but differ in structural syntax. This page covers the essentials of both.

## Terminology at a Glance

- **typed**: the overall design idea that text values carry explicit type semantics in their encoding.
- **typed types**: the 12 built-in value encoding categories.
- **typed unit**: one concrete encoded value such as `123.45m`, `2026-03-08`, or `uuid(...)`.
- `hex` and `b64` are listed separately because they are distinct typed types, even though both represent binary payloads.

## T-TOON Syntax

T-TOON uses indentation-based structure with no redundant brackets. It is designed for human reading and manual editing.

### Object

```text
name: "Alice"
age: 30
tags[2]: "admin", "ops"
```

### Nested Object

```text
user:
  name: "Alice"
  address:
    city: "Taipei"
```

### Tabular (List of Uniform Objects)

```text
[2]{name,score}:
"Alice", 95
"Bob", 87
```

The header `[N]{fields}:` declares row count and column names. Delimiters can be comma (default), tab, or pipe.

## T-JSON Syntax

T-JSON uses JSON-like `{}` / `[]` brackets. Object keys must be quoted strings (following JSON rules). The value layer uses typed syntax instead of plain JSON values.

### Object

```text
{"name": "Alice", "amount": 123.45m, "id": uuid(550e8400-e29b-41d4-a716-446655440000)}
```

### Array

```text
[1, 2, 3]
```

### Nested

```text
{"user": {"name": "Alice", "scores": [95, 87]}}
```

## Typed Value System

Both syntaxes share the same 12 typed value encodings, that is, the same set of `typed types`:

| Type | Example | Notes |
| :--- | :--- | :--- |
| `null` | `null` | Null value |
| `bool` | `true` / `false` | Lowercase keywords |
| `int` | `42` / `-1_000` | Sign prefixes and `_` separators allowed |
| `float` | `3.14` / `1e-9` / `inf` / `nan` | Floating point with scientific notation and special values |
| `decimal` | `123.45m` | Lowercase `m` suffix; no scientific notation; full precision preserved |
| `string` | `"Alice"` | Always double-quoted |
| `date` | `2026-03-08` | `YYYY-MM-DD` |
| `time` | `14:30:00.123456` | Up to microsecond precision |
| `datetime` | `2026-03-08T14:30:00+08:00` | ISO 8601 with optional timezone |
| `uuid` | `uuid(550e8400-e29b-41d4-a716-446655440000)` | Wrapper to prevent misidentification as string |
| `hex(...)` | `hex(4A42)` | Hexadecimal binary encoding |
| `b64(...)` | `b64(SkI=)` | Base64 binary encoding |

### Why These 12 Typed Types

TTOON `typed types` are not a copy of any single database or language type system. They are chosen as the practical intersection across mainstream RDBMSs, Arrow, and the Python / JS / Rust SDKs.

The goal is to keep a type layer that is:

- sufficient for common cross-system data interchange
- stable across both the object path and the Arrow path
- portable without binding the format to vendor-specific database types

For that reason, types such as `jsonb`, `enum`, `interval`, `array`, geospatial types, or `money` are not built-in typed types for now. They should be handled by higher-level schema conventions or custom codecs.

### Key Rules

- **Strings are always quoted**: All string values use double quotes `"..."` to eliminate bare token ambiguity.
- **Decimal uses `m` suffix**: `123.45m` — this distinguishes exact decimal from floating point `123.45`.
- **UUID and binary use wrappers**: `uuid(...)`, `hex(...)`, `b64(...)` — preventing misidentification as strings.

## Escape Rules

### T-TOON

T-TOON allows only 5 escape sequences: `\\`, `\"`, `\n`, `\r`, `\t`. Any other escape (e.g., `\uXXXX`) is rejected.

### T-JSON

T-JSON follows the full JSON escape ruleset, including `\uXXXX` Unicode escapes, `\b`, `\f`, etc.

## Format Detection

TTOON auto-detects the input format:

- First non-whitespace character is `{` → **T-JSON**
- First line matches tabular header `[N]{fields}:` or `[N]:` → **T-TOON** tabular
- First non-whitespace character is `[` but it does not match a tabular header → **T-JSON**
- Otherwise → `typed_unit` from `detect_format()`; the T-TOON parser then distinguishes indentation-based structure from a single typed value

Once a format is determined, the parser does not fall back to another format.

## When to Use Which

| Scenario | Recommended |
| :--- | :--- |
| Human-readable config, logs, or diffs | T-TOON |
| Large tabular datasets | T-TOON tabular |
| Interop with JSON-based systems | T-JSON |
| Bracket-based structure required | T-JSON |
| Cross-language object exchange | Either (auto-detected on parse) |

## Next Steps

- **[Why TTOON?](../concepts/why-ttoon.md)** — Deeper motivation and positioning
- **[T-TOON vs T-JSON](../concepts/ttoon-vs-tjson.md)** — Detailed comparison
- **[Typed Values](../concepts/typed-values.md)** — Complete type reference and cross-language behavior
- **[Type Mapping](../reference/type-mapping.md)** — Type mapping across SDKs, Arrow, and mainstream RDBMSs
