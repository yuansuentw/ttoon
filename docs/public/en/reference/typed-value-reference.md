---
title: Typed Value Reference
sidebar_position: 2
sidebar_label: Typed Values
description: Precise reference for TTOON typed values, runtime mappings, Arrow mappings, and RDBMS correspondence.
---

# Typed Value Reference

This page is the detailed reference for TTOON's typed value layer. If you only need a beginner introduction to the two syntaxes, start with [Format Overview](../getting-started/format-overview.md).

At the structure layer, T-TOON extends [TOON](https://toonformat.dev/) from the [toon-format project](https://github.com/toon-format/toon). This page focuses on what TTOON adds on top of that base: the typed value layer and its cross-language semantics.

## Terminology

- **typed**: the overall design concept that the text encoding of a value carries type semantics directly
- **typed types**: the 12 built-in value encodings
- **typed unit**: one serialized value fragment that the parser sees directly, such as `123.45m`, `2026-03-08`, or `uuid(...)`
- `hex` and `b64` both represent binary payloads, but they still count as two separate typed types at the text-format level

## Typed Value System

Both syntaxes share the same 12 typed value encodings:

| Type | Syntax | Examples | Notes |
| :--- | :--- | :--- | :--- |
| `null` | Keyword | `null` | Null value |
| `bool` | Keyword | `true`, `false` | Lowercase only |
| `int` | Digits with optional sign and `_` separators | `42`, `-1_000`, `0` | Sign prefixes and `_` separators allowed |
| `float` | Decimal point or scientific notation | `3.14`, `1e-9`, `-0.5`, `inf`, `-inf`, `nan` | Special values: `inf`, `-inf`, `nan` |
| `decimal` | Digits with `m` suffix | `123.45m`, `0.00m`, `-99.9m` | Lowercase `m`; no scientific notation |
| `string` | Double-quoted | `"Alice"`, `"hello\nworld"` | Always double-quoted |
| `date` | `YYYY-MM-DD` | `2026-03-08` | Bare in both syntaxes |
| `time` | `HH:MM:SS[.fractional]` | `14:30:00`, `14:30:00.123456` | Up to microsecond precision |
| `datetime` | ISO 8601 | `2026-03-08T14:30:00`, `2026-03-08T14:30:00+08:00`, `2026-03-08T14:30:00Z` | Optional timezone |
| `uuid` | `uuid(...)` wrapper | `uuid(550e8400-e29b-41d4-a716-446655440000)` | Lowercase hex only |
| `hex` | `hex(...)` wrapper | `hex(4A42)`, `hex(48656C6C6F)` | Hexadecimal binary encoding |
| `b64` | `b64(...)` wrapper | `b64(SkI=)`, `b64(SGVsbG8=)` | Base64 binary encoding |

### Why These 12 Typed Types

TTOON typed types are not copied from any single database or language type system. They are chosen as the practical intersection across mainstream RDBMSs, Arrow, and the Python / JS / Rust SDKs.

The goal is a type layer that is:

- sufficient for common cross-system data interchange
- stable across both the object path and the Arrow path
- portable without binding the format to vendor-specific database types

For that reason, types such as `jsonb`, `enum`, `interval`, `array`, geospatial types, or `money` are not built-in typed types. They should be handled by higher-level schema conventions or custom codecs.

### Key Rules

- **Strings are always quoted**: all string values use double quotes `"..."` to eliminate bare-token ambiguity
- **Decimal uses `m` suffix**: `123.45m` is exact decimal; `123.45` is float
- **UUID and binary use wrappers**: `uuid(...)`, `hex(...)`, `b64(...)`

## Type Details

### null

`null` is a single keyword representing the absence of a value.

### bool

`true` and `false` are the only valid boolean forms. They must be lowercase.

### int

Integer values allow an optional sign prefix and `_` digit separators:

```text
42
-1_000_000
0
```

JS precision note: JS native `number` safely represents integers only within `-(2^53 - 1)` to `2^53 - 1`. Values outside this range throw by default. Use `intBigInt()` or `intNumber({ overflow: 'lossy' })` as needed. See [JS Codecs & Int64](../guides/js-codecs-and-int64.md).

### float

Floating-point values must contain a decimal point or use scientific notation:

```text
3.14
1e-9
-0.5
inf
-inf
nan
```

### decimal

Exact decimal values use a lowercase `m` suffix:

```text
123.45m
0.00m
-99.9m
```

Scientific notation is not used for decimal. `123.45m` is decimal; `123.45` is float.

### string

Strings are always double-quoted:

```text
"Alice"
"hello world"
""
```

Escape rules:

- **T-TOON**: only `\\`, `\"`, `\n`, `\r`, `\t` are allowed
- **T-JSON**: full JSON escape set, including `\uXXXX`, `\b`, `\f`

### date

`YYYY-MM-DD` format, bare in both syntaxes:

```text
2026-03-08
```

### time

`HH:MM:SS` with optional fractional seconds up to microsecond precision:

```text
14:30:00
14:30:00.123456
```

### datetime

ISO 8601 format with optional timezone:

```text
2026-03-08T14:30:00
2026-03-08T14:30:00+08:00
2026-03-08T14:30:00Z
```

### uuid

UUID values use `uuid(...)`:

```text
uuid(550e8400-e29b-41d4-a716-446655440000)
```

The UUID must use the standard 8-4-4-4-12 shape and lowercase hex only.

### hex / b64

Binary payloads can use either hexadecimal or Base64 text:

```text
hex(48656C6C6F)
b64(SGVsbG8=)
```

`hex` and `b64` map to the same runtime concept of binary data. The difference exists only in the text encoding.

## Cross-Language Mapping (Object Path)

| Typed Type | Python `loads()` | JS `parse()` Default | JS with Codec |
| :--- | :--- | :--- | :--- |
| `null` | `None` | `null` | — |
| `bool` | `bool` | `boolean` | — |
| `int` | `int` | `number` (throws if unsafe) | `bigint` via `intBigInt()` |
| `float` | `float` | `number` | — |
| `decimal` | `decimal.Decimal` | `string` (stripped `m`) | `Decimal` class via codec |
| `string` | `str` | `string` | — |
| `date` | `datetime.date` | `string` | Custom via codec |
| `time` | `datetime.time` | `string` | Custom via codec |
| `datetime` | `datetime.datetime` | `string` | Custom via codec |
| `uuid` | `uuid.UUID` | `string` | Custom via codec |
| `hex`/`b64` | `bytes` | `Uint8Array` | Custom via codec |

### Why JS Returns Strings for Some Types

JS has no native `Decimal`, `UUID`, date-only, or time-only types. Returning strings by default avoids:

- forcing third-party dependencies
- divergent assumptions between browser and Node.js runtimes

Use `use()` to register richer codecs. See [JS Codecs & Int64](../guides/js-codecs-and-int64.md).

## Arrow Path

| Typed Type | Python `read_arrow()` | JS `readArrow()` | Arrow Native Type |
| :--- | :--- | :--- | :--- |
| `null` | Nullable column | Nullable column | `Null` or nullable typed column |
| `bool` | `Boolean` | `Bool` | `Boolean` |
| `int` | `Int64` | `Int64` | `Int64` |
| `float` | `Float64` | `Float64` | `Float64` |
| `decimal` | `Decimal128/256` | `Decimal` | `Decimal128` or `Decimal256` |
| `string` | `Utf8` | `Utf8` | `Utf8` |
| `date` | `Date32` | `DateDay` | `Date32` |
| `time` | `Time64(Microsecond)` | `TimeMicrosecond` | `Time64(Microsecond)` |
| `datetime` | `Timestamp(μs[, tz])` | `TimestampMicrosecond` | `Timestamp(Microsecond[, tz])` |
| `uuid` | `FixedSizeBinary(16)` | `FixedSizeBinary(16)` | `FixedSizeBinary(16)` + UUID metadata |
| `hex`/`b64` | `Binary` | `Binary` | `Binary` / `LargeBinary` / `FixedSizeBinary` |

### Arrow Type Preservation

- `decimal` uses `Decimal128` or `Decimal256`, not `Utf8`
- `uuid` uses `FixedSizeBinary(16)` with UUID metadata, not `Utf8`
- `datetime` preserves timezone information when present
- all-null columns infer as `Null`

### Datetime Timezone Behavior

- timezone-aware and naive datetimes cannot be mixed in the same column
- the JS Arrow bridge enforces this and raises a schema inference error on mixing
- timezone-aware `datetime` uses `Timestamp(Microsecond, tz)`
- naive `datetime` uses `Timestamp(Microsecond)`

### Practical Read / Write Guidance

- `decimal` should usually remain an exact numeric database column instead of being downgraded to float
- `uuid` can use native types directly in PostgreSQL and SQL Server; MySQL and SQLite commonly rely on `binary(16)` or text conventions
- `date` / `time` / `datetime` in SQLite are usually application-level conventions, not strong native types
- `hex` and `b64` usually map back to the same binary database column type

## Mainstream RDBMS Mapping

| TTOON typed type | Semantic meaning | PostgreSQL | MySQL / MariaDB | SQLite | SQL Server | Notes |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| `null` | missing value | `NULL` | `NULL` | `NULL` | `NULL` | universal |
| `bool` | boolean flag | `boolean` | `boolean` / `tinyint(1)` | `INTEGER` 0/1 or `BOOLEAN` alias | `bit` | SQLite has no separate boolean storage class |
| `int` | signed integer | `smallint` / `integer` / `bigint` | `tinyint` / `smallint` / `int` / `bigint` | `INTEGER` | `smallint` / `int` / `bigint` | TTOON does not define unsigned-only semantics |
| `float` | approximate numeric | `real` / `double precision` | `float` / `double` | `REAL` | `real` / `float` | approximate, not exact decimal |
| `decimal` | exact base-10 numeric | `numeric` / `decimal` | `decimal` / `numeric` | commonly `NUMERIC`, `TEXT`, or app-level decimal | `decimal` / `numeric` | SQLite has no fixed-precision decimal storage class |
| `string` | text | `text` / `varchar` | `char` / `varchar` / `text` | `TEXT` | `nvarchar` / `varchar` / `nchar` / `char` | canonical TTOON form is UTF-8 text |
| `date` | calendar date | `date` | `date` | commonly `TEXT` (`YYYY-MM-DD`) | `date` | SQLite is convention-based here |
| `time` | wall-clock time | `time` | `time` | commonly `TEXT` (`HH:MM:SS[.ffffff]`) | `time` | no date component |
| `datetime` | timestamp / date-time | `timestamp` / `timestamptz` | `datetime` / `timestamp` | commonly ISO 8601 `TEXT` or Unix time `INTEGER` | `datetime2` / `datetimeoffset` | timezone is preserved when supported by source type |
| `uuid` | 128-bit identifier | `uuid` | commonly `binary(16)` or `char(36)` | commonly `TEXT` or `BLOB` | `uniqueidentifier` | MySQL and SQLite do not have a universal native UUID type |
| `hex` | binary payload | `bytea` | `binary` / `varbinary` / `blob` | `BLOB` | `binary` / `varbinary` | text form uses hexadecimal |
| `b64` | binary payload | `bytea` | `binary` / `varbinary` / `blob` | `BLOB` | `binary` / `varbinary` | text form uses Base64 |

## Serialization Direction

| Python Type | Typed Output |
| :--- | :--- |
| `None` | `null` |
| `bool` | `true` / `false` |
| `int` | `42` |
| `float` | `3.14` |
| `decimal.Decimal` | `123.45m` |
| `str` | `"Alice"` |
| `datetime.date` | `2026-03-08` |
| `datetime.time` | `14:30:00` |
| `datetime.datetime` | `2026-03-08T14:30:00` |
| `uuid.UUID` | `uuid(550e8400-...)` |
| `bytes` | `hex(...)` or `b64(...)` |

| JS Value | Typed Output |
| :--- | :--- |
| `null` | `null` |
| `boolean` | `true` / `false` |
| `number` (safe integer) | `42` |
| `number` (float) | `3.14` |
| `string` | `"Alice"` |
| `bigint` (signed i64 range) | `42` |
| `Date` | `2026-03-08T14:30:00.000Z` |
| `Array` | `[1, 2]` |
| `plain object` | `{"name": "Alice"}` |
| `Uint8Array` | `hex(...)` or `b64(...)` |
| `toon.decimal('123.45')` | `123.45m` |
| `toon.uuid('...')` | `uuid(...)` |
| `toon.date('...')` | `2026-03-08` |
| `toon.time('14:30:00')` | `14:30:00` |
| `toon.datetime('2026-03-08T14:30:00+08:00')` | `2026-03-08T14:30:00+08:00` |

JS `Date` values are serialized via `Date.toISOString()`, so the output is UTC and includes millisecond precision.

## Related Pages

- **[Format Detection](./format-detection.md)** — Exact auto-detection rules and parser routing
- **[API Matrix](./api-matrix.md)** — Batch, streaming, Arrow, and transcode capability matrix
- **[JS Codecs & Int64](../guides/js-codecs-and-int64.md)** — How to override JS default type mapping
