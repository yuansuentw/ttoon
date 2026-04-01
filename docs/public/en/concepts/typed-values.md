---
title: Typed Values
sidebar_position: 3
sidebar_label: Typed Values
description: Complete reference for the 12 typed value encodings shared by T-TOON and T-JSON.
---

# Typed Values

TTOON defines 12 typed value encodings — called **typed types** — that are shared between T-TOON and T-JSON syntaxes. A single encoded instance is called a **typed unit** (e.g., `true`, `2026-03-01`, `uuid(...)`).

## Terminology

- **typed**: the overall design concept that the text encoding of a value carries type semantics directly.
- **typed types**: the set of 12 built-in value encodings.
- **typed unit**: one serialized value fragment, meaning the concrete value text the parser sees directly.
- Although `hex` and `b64` both represent binary payloads, they still count as separate typed types at the text-format level.

## Type Table

| Type | Syntax | Examples |
| :--- | :--- | :--- |
| `null` | Keyword | `null` |
| `bool` | Keyword | `true`, `false` |
| `int` | Digits with optional sign and `_` separators | `42`, `-1_000`, `0` |
| `float` | Decimal point or scientific notation | `3.14`, `1e-9`, `-0.5`, `inf`, `-inf`, `nan` |
| `decimal` | Digits with `m` suffix | `123.45m`, `0.00m`, `-99.9m` |
| `string` | Double-quoted | `"Alice"`, `"hello\nworld"` |
| `date` | `YYYY-MM-DD` | `2026-03-08` |
| `time` | `HH:MM:SS[.fractional]` | `14:30:00`, `14:30:00.123456` |
| `datetime` | ISO 8601 | `2026-03-08T14:30:00`, `2026-03-08T14:30:00+08:00` |
| `uuid` | `uuid(...)` wrapper | `uuid(550e8400-e29b-41d4-a716-446655440000)` |
| `hex` | `hex(...)` wrapper | `hex(4A42)` |
| `b64` | `b64(...)` wrapper | `b64(SkI=)` |

## Type Details

### null

`null` — a single keyword. Represents the absence of a value.

### bool

`true` or `false` — lowercase only. No other truthy/falsy representations.

### int

Integer values with optional sign prefix and `_` digit separators for readability.

```text
42
-1_000_000
0
```

**JS precision note**: JS native `number` safely represents integers only within `-(2^53 - 1)` to `2^53 - 1`. Values outside this range throw an error by default. Use `intBigInt()` codec to receive `BigInt`, or `intNumber({ overflow: 'lossy' })` to explicitly accept precision loss. See [JS Codecs & Int64](../guides/js-codecs-and-int64.md).

### float

Floating-point values. Must contain a decimal point or use scientific notation.

```text
3.14
1e-9
-0.5
inf
-inf
nan
```

### decimal

Exact decimal values with lowercase `m` suffix. No scientific notation — full precision is preserved.

```text
123.45m
0.00m
-99.9m
```

The `m` suffix distinguishes decimal from float: `123.45` is float, `123.45m` is decimal.

### string

Always double-quoted. Never bare (unknown bare tokens in `compat` mode are accepted as strings).

```text
"Alice"
"hello world"
""
```

**T-TOON escape rules**: Only `\\`, `\"`, `\n`, `\r`, `\t` are allowed. Other escapes (e.g., `\uXXXX`) are rejected.

**T-JSON escape rules**: Full JSON escape set including `\uXXXX`, `\b`, `\f`.

### date

`YYYY-MM-DD` format. Bare (unquoted) in both T-TOON and T-JSON.

```text
2026-03-08
```

### time

`HH:MM:SS` with optional fractional seconds up to microsecond precision.

```text
14:30:00
14:30:00.123456
```

### datetime

ISO 8601 format with optional timezone.

```text
2026-03-08T14:30:00
2026-03-08T14:30:00+08:00
2026-03-08T14:30:00Z
```

### uuid

Wrapped in `uuid(...)` to prevent misidentification as a plain string.

```text
uuid(550e8400-e29b-41d4-a716-446655440000)
```

The UUID must be 36 characters in the standard 8-4-4-4-12 format.
Lowercase hex only; uppercase hex is rejected.

### hex / b64 (binary payloads)

Binary data uses either hexadecimal or base64 encoding:

```text
hex(48656C6C6F)
b64(SGVsbG8=)
```

`hex` and `b64` both represent binary data, but in TTOON terminology they still count as two separate typed types. They share the same runtime mapping (`bytes`, `Uint8Array`, Arrow `Binary`); the difference exists only in the text encoding.

## Cross-Language Mapping

| Typed Type | Python `loads()` | JS `parse()` Default | Arrow Schema |
| :--- | :--- | :--- | :--- |
| `null` | `None` | `null` | Nullable column |
| `bool` | `bool` | `boolean` | `Boolean` |
| `int` | `int` | `number` (error if overflow) | `Int64` |
| `float` | `float` | `number` | `Float64` |
| `decimal` | `decimal.Decimal` | `string` (strip `m` suffix) | `Decimal128/256` |
| `string` | `str` | `string` | `Utf8` |
| `date` | `datetime.date` | `string` | `Date32` |
| `time` | `datetime.time` | `string` | `Time64(Microsecond)` |
| `datetime` | `datetime.datetime` | `string` | `Timestamp(Microsecond[, tz])` |
| `uuid` | `uuid.UUID` | `string` | `FixedSizeBinary(16)` + UUID metadata |
| `hex`/`b64` | `bytes` | `Uint8Array` | `Binary` |

**Why JS returns strings for some types**: JS has no native `Decimal`, `Date` (with time-only), or `UUID` types. Returning strings avoids forcing third-party dependencies. Codecs can override this mapping — see [JS Codecs & Int64](../guides/js-codecs-and-int64.md).

For the complete cross-language type matrix including Arrow details, see [Type Mapping](../reference/type-mapping.md).
