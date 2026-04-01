---
title: Troubleshooting
sidebar_position: 7
sidebar_label: Troubleshooting
description: Common TTOON errors and their fixes.
---

# Troubleshooting

## Common Errors

| Error | Common Cause | Fix |
| :--- | :--- | :--- |
| `unknown bare token` | Unquoted string in `strict` mode | Add quotes: `"text"`, or switch to `compat` mode |
| `object keys must be strings` | T-JSON key without double quotes | Use `{"key": 1}` instead of `{key: 1}` |
| `line-separated rows are not supported` | Bare comma-separated lines without tabular header | Use T-TOON tabular `[N]{fields}:` or T-JSON array |
| `read_arrow: input must be a list` | Passed an object or scalar to Arrow API | Restructure as list of uniform objects |
| `field 'x' has inconsistent types` | Different scalar types in the same column | Normalize field types before input |
| `Int64 value ... outside JS safe integer range` | JS `number` cannot safely represent int64 | Use `intBigInt()` or `intNumber({ overflow: 'lossy' })` |
| `invalid escape sequence in T-TOON string` | Used unsupported escape (e.g., `\uXXXX`) | T-TOON only allows `\\` `\"` `\n` `\r` `\t`; use T-JSON for full escapes |
| `unknown delimiter` / `unknown binary_format` | Unsupported option value | Delimiter: `","`, `"\t"`, `"\|"` only; binary format: `"hex"`, `"b64"` only |

## Parse Mode Issues

### `strict` mode rejects bare strings

```python
# Fails — "hello" is a bare token in strict mode
ttoon.loads("key: hello", mode="strict")

# Works — string is quoted
ttoon.loads('key: "hello"', mode="strict")
```

`strict` mode is best for machine-generated data. Use `compat` for hand-typed content.

### `mode` only affects T-TOON

The `mode` parameter has no effect on T-JSON parsing — T-JSON is always strict. This is by design, since T-JSON follows JSON structural rules.

## Arrow Issues

### Not all data is arrowable

`read_arrow()` rejects:
- Non-list root values (single objects, scalars)
- Objects containing nested lists or objects as field values
- Columns with mixed types (e.g., `int` in some rows, `string` in others)

### Datetime timezone mixing

The JS Arrow bridge rejects columns that mix timezone-aware and naive datetimes. Ensure all datetimes in a column are either all timezone-aware or all naive.

## JS-Specific Issues

### `apache-arrow` not installed

`readArrow()`, `stringifyArrow()`, and `stringifyArrowTjson()` require the optional peer dependency `apache-arrow`:

```bash
npm install apache-arrow
```

### Codec not taking effect

- Codecs registered via `use()` are global. Ensure `await use(...)` completes before parsing.
- Per-call `codecs` in `ParseOptions` override global codecs for that call only.
- Codecs do not affect Arrow paths — only the object path (`parse()` / `stringify()`).

### TranscodeError wrapping

`tjsonToTtoon()` and `ttoonToTjson()` wrap lower-level failures in `TranscodeError`. In JS, `e.phase` is currently always reported as `'parse'`, so prefer `e.operation`, `e.sourceKind`, and `e.source.message` for diagnostics:

```ts
try {
  tjsonToTtoon(text);
} catch (e) {
  if (e instanceof TranscodeError) {
    console.log(e.operation); // 'tjson_to_ttoon'
    console.log(e.phase);     // currently always 'parse' in JS
    console.log(e.sourceKind); // underlying source error kind
  }
}
```

## Python-Specific Issues

### Polars/PyArrow not detected

`dumps()` auto-detects Polars DataFrame and PyArrow Table/RecordBatch. If detection fails:

- Ensure `polars` and/or `pyarrow` are installed
- Pass Arrow tables explicitly to `read_arrow()` or `stringify_arrow_tjson()`

### `to_tjson()` does not accept Arrow input

Use `stringify_arrow_tjson()` for Arrow/Polars → T-JSON conversion. `to_tjson()` only accepts Python native objects.

## Usage Guidelines

| Scenario | Recommendation |
| :--- | :--- |
| General object exchange | Prefer T-TOON |
| Need bracket-based structure | Use T-JSON |
| Large tables and DataFrames | Use Arrow / Polars path |
| JS with potential int64 | Decide on `bigint` or overflow strategy before production |
| Human-editable data | Use `compat` mode |
| Machine-generated data | Use `strict` mode |
