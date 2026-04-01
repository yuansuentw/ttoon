---
title: Behaviors & Limitations
sidebar_position: 6
sidebar_label: Behaviors & Limitations
description: Edge cases, constraints, and design-level behaviors of TTOON.
---

# Behaviors & Limitations

## Empty Input

Empty strings, whitespace-only, or newline-only input currently parse to an empty object `{}`, not `null`.

## Strings Must Be Quoted

All string values are explicitly double-quoted in both T-TOON and T-JSON. This eliminates bare token ambiguity — there is no "unquoted string" type.

T-JSON object keys must also be quoted strings, following JSON rules.

## Line-Separated Rows

Bare comma-separated lines without a tabular header are **not** a valid T-TOON format:

```text
1, 2, 3
4, 5, 6
```

Use `T-TOON` tabular with `[N]{fields}:` header or `T-JSON` arrays instead.

## Arrow Input Requirements

`read_arrow()` across all languages enforces:

- Root must be a list
- Each element must be an object
- Field types must be consistent across rows (same scalar type per column)
- Structural fields (list / object) are not arrowable

## JS int64 Precision

JS native `number` safely represents integers only within `-(2^53 - 1)` to `2^53 - 1`.

- **Default behavior**: throws an error if the value exceeds the safe range — no silent precision loss
- **`intBigInt()`**: maps all integers to `BigInt`
- **`intNumber({ overflow: 'lossy' })`**: explicitly accepts precision loss

## JS Default String Strategy

JS returns `string` by default for `decimal`, `date`, `time`, `datetime`, and `uuid`. This avoids locking users into specific third-party dependencies. Register codecs with `use()` to change this behavior.

## Codec Scope

Codecs are language-specific object-path adaptation layers:

- **JavaScript**: affect `parse()`, `stringify()`, `toTjson()`, and object-path streaming readers/writers
- **Python**: affect object-path streaming readers/writers only

They do **not** affect:

- T-TOON / T-JSON syntax
- Rust behavior
- Arrow schema inference rules
- Arrow streaming APIs
- Direct transcode APIs (`tjson_to_ttoon()` / `ttoon_to_tjson()`, `tjsonToTtoon()` / `ttoonToTjson()`)

## Arrow Datetime Timezone Consistency

The JS Arrow bridge does not allow mixing timezone-aware and naive datetimes within the same column. Mixing them causes a schema inference error.

## Format Detection is Commitment

Once `detect_format()` determines the input is T-TOON or T-JSON, the parser commits to that format. There is no silent fallback to another format on parse error.

## Parse is Validation

TTOON has no separate `validate()` API. Type validity is checked during parsing:

- UUID format correctness (36-char, 8-4-4-4-12)
- Decimal `m` suffix presence
- T-TOON string escape compliance (only 5 allowed)
- T-JSON object key is a string
- Consistent field types for Arrow path

## T-TOON vs T-JSON Escape Asymmetry

T-TOON supports only 5 escape sequences: `\\`, `\"`, `\n`, `\r`, `\t`. T-JSON supports the full JSON escape set including `\uXXXX`, `\b`, `\f`. Using T-JSON escapes in T-TOON text will cause a parse error.

## Streaming Header Convention

- **T-TOON batch**: `[N]{fields}:` — `N` is the exact row count
- **T-TOON streaming**: `[*]{fields}:` — `*` indicates unbounded rows
- **T-JSON streaming**: top-level array of objects, schema-known values limited to scalars and null
