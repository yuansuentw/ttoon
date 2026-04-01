---
title: Type Mapping
sidebar_position: 5
sidebar_label: Type Mapping
description: Cross-language type conversion table for Python, JavaScript, Rust, and Arrow.
---

# Type Mapping

This page explains how TTOON `typed types` map across languages, Arrow, and mainstream RDBMSs.

## Design Principle

TTOON `typed types` form an interchange layer, not a full mirror of any one database type system. They are intentionally chosen as the most common and stable intersection across PostgreSQL, MySQL / MariaDB, SQLite, and SQL Server.

That means:

- TTOON focuses on shared semantics across languages and databases, not every vendor-specific detail
- the same typed type may map to different physical column types in different databases
- SQLite uses a dynamic type system, so many mappings there are storage conventions rather than strict column types

## Mainstream RDBMS Mapping

| TTOON typed type | Semantic meaning | PostgreSQL | MySQL / MariaDB | SQLite | SQL Server | Notes |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| `null` | missing value | `NULL` | `NULL` | `NULL` | `NULL` | universal |
| `bool` | boolean flag | `boolean` | `boolean` / `tinyint(1)` | `INTEGER` 0/1 or `BOOLEAN` alias | `bit` | SQLite has no separate boolean storage class |
| `int` | signed integer | `smallint` / `integer` / `bigint` | `tinyint` / `smallint` / `int` / `bigint` | `INTEGER` | `smallint` / `int` / `bigint` | TTOON does not define unsigned-only semantics |
| `float` | approximate numeric | `real` / `double precision` | `float` / `double` | `REAL` | `real` / `float` | approximate, not exact decimal |
| `decimal` | exact base-10 numeric | `numeric` / `decimal` | `decimal` / `numeric` | commonly handled via `NUMERIC` convention, `TEXT`, or app-level decimal | `decimal` / `numeric` | SQLite has no fixed-precision decimal storage class |
| `string` | text | `text` / `varchar` | `char` / `varchar` / `text` | `TEXT` | `nvarchar` / `varchar` / `nchar` / `char` | canonical TTOON form is UTF-8 text |
| `date` | calendar date | `date` | `date` | commonly `TEXT` (`YYYY-MM-DD`) | `date` | SQLite is convention-based here |
| `time` | wall-clock time | `time` | `time` | commonly `TEXT` (`HH:MM:SS[.ffffff]`) | `time` | no date component; timezone support depends on source system |
| `datetime` | timestamp / date-time | `timestamp` / `timestamptz` | `datetime` / `timestamp` | commonly ISO 8601 `TEXT` or Unix time `INTEGER` | `datetime2` / `datetimeoffset` | timezone is preserved when the source type supports it |
| `uuid` | 128-bit identifier | `uuid` | commonly `binary(16)` or `char(36)` | commonly `TEXT` or `BLOB` | `uniqueidentifier` | MySQL / SQLite do not have a universal native UUID column type |
| `hex` | binary payload | `bytea` | `binary` / `varbinary` / `blob` | `BLOB` | `binary` / `varbinary` | text form uses hexadecimal |
| `b64` | binary payload | `bytea` | `binary` / `varbinary` / `blob` | `BLOB` | `binary` / `varbinary` | text form uses Base64 |

## Object Path

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

For JS serialization inputs such as `bigint`, `Date`, arrays, and plain objects, see the Serialization Direction section below.

### Why JS Returns Strings for Some Types

JS has no native `Decimal`, `UUID`, date-only, or time-only types. Returning strings by default avoids:

- Forcing third-party dependencies (`decimal.js`, `moment.js`)
- Divergent runtime assumptions between browser and Node.js

Use `use()` to register codecs for richer types. See [JS Codecs & Int64](../guides/js-codecs-and-int64.md).

## Arrow Path

| Typed Type | Python `read_arrow()` | JS `readArrow()` | Arrow Native Type |
| :--- | :--- | :--- | :--- |
| `null` | Nullable column | Nullable column | `Null` or nullable typed column |
| `bool` | `Boolean` | `Bool` | `Boolean` |
| `int` | `Int64` | `Int64` | `Int64` |
| `float` | `Float64` | `Float64` | `Float64` |
| `decimal` | `Decimal128/256` | `Decimal` | `Decimal128` or `Decimal256` (by precision) |
| `string` | `Utf8` | `Utf8` | `Utf8` |
| `date` | `Date32` | `DateDay` | `Date32` |
| `time` | `Time64(Microsecond)` | `TimeMicrosecond` | `Time64(Microsecond)` |
| `datetime` | `Timestamp(μs[, tz])` | `TimestampMicrosecond` | `Timestamp(Microsecond[, tz])` |
| `uuid` | `FixedSizeBinary(16)` | `FixedSizeBinary(16)` | `FixedSizeBinary(16)` + UUID metadata |
| `hex`/`b64` | `Binary` | `Binary` | `Binary` / `LargeBinary` / `FixedSizeBinary` |

### Practical Read / Write Guidance

- `decimal` should usually remain an exact numeric database column instead of being downgraded to floating point
- `uuid` can use native types directly in PostgreSQL and SQL Server; MySQL and SQLite commonly rely on schema conventions such as `binary(16)` or text form
- `date` / `time` / `datetime` in SQLite are almost always application-level conventions, not the same kind of strong typing you get in PostgreSQL
- `hex` and `b64` usually map back to the same binary column type in the database layer; the difference exists only in the TTOON text encoding

### Arrow Type Preservation

Arrow types are preserved at their native resolution:

- `decimal` uses `Decimal128` or `Decimal256` — never downgraded to `Utf8`
- `uuid` uses `FixedSizeBinary(16)` with UUID metadata — never downgraded to `Utf8`
- `datetime` timezone information is preserved when present
- `null` columns with all-null values infer as `Null` type

### Datetime Timezone Behavior

- Timezone-aware and naive datetimes cannot be mixed within the same column
- The JS Arrow bridge enforces this and raises a schema inference error on mixing
- `datetime` with timezone uses `Timestamp(Microsecond, tz)`
- `datetime` without timezone uses `Timestamp(Microsecond)` (naive)

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
| `Date` | `2026-03-08T14:30:00Z` |
| `Array` | `[1, 2]` |
| `plain object` | `{"name": "Alice"}` |
| `Uint8Array` | `hex(...)` or `b64(...)` |
| `toon.decimal('123.45')` | `123.45m` |
| `toon.uuid('...')` | `uuid(...)` |
| `toon.date('...')` | `2026-03-08` |
| `toon.time('14:30:00')` | `14:30:00` |
| `toon.datetime('2026-03-08T14:30:00+08:00')` | `2026-03-08T14:30:00+08:00` |
