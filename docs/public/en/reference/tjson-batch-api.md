---
title: T-JSON Batch API
sidebar_position: 4
sidebar_label: T-JSON Batch API
description: Batch APIs for reading, writing, and transcoding T-JSON across Python, JavaScript, and Rust.
---

# T-JSON Batch API

This page groups the non-streaming APIs that produce or consume T-JSON text. Use the language tabs to switch between Python, JavaScript, and Rust.

Batch parse APIs still auto-detect `T-TOON`, `T-JSON`, and `typed_unit` input. The functions listed here are grouped by their T-JSON use case, not by exclusive parser support.

import Tabs from '@theme/Tabs';
import TabItem from '@theme/TabItem';

<Tabs groupId="language">
<TabItem value="python" label="Python">

Package: `ttoon`

## Read T-JSON Batch Text

### `loads(text, mode=None) -> object`

- Parses `T-JSON`, `T-TOON`, or `typed_unit` text to Python native objects
- `mode` only affects the T-TOON parse path

### `read_arrow(text) -> pyarrow.Table`

- Parses batch text to `pyarrow.Table`
- Auto-detects format
- Input must be a list of uniform objects with scalar fields

## Write T-JSON Batch Text

### `to_tjson(obj, binary_format=None) -> str`

- Serializes Python objects to T-JSON text
- Does not accept Arrow / Polars input

### `stringify_arrow_tjson(obj, binary_format=None) -> str`

- Serializes `pyarrow.Table`, `pyarrow.RecordBatch`, or `polars.DataFrame` to T-JSON list-of-objects

## Transcode into T-JSON

### `ttoon_to_tjson(text, *, mode="compat", binary_format=None) -> str`

- Converts T-TOON text directly to T-JSON through Rust IR only
- `mode`: `"compat"` (default) or `"strict"`

## T-JSON Batch Options

| Parameter | APIs | Values | Default |
| :--- | :--- | :--- | :--- |
| `binary_format` | `to_tjson`, `stringify_arrow_tjson`, `ttoon_to_tjson` | `"hex"`, `"b64"` | `"hex"` |
| `mode` | `loads`, `ttoon_to_tjson` | `"compat"`, `"strict"` | `"compat"` |

## Related Utilities

- `detect_format(text) -> str` is documented on [Format Detection](./format-detection.md)
- `TranscodeError` details remain the same regardless of which batch format you target

</TabItem>
<TabItem value="js" label="JavaScript / TypeScript">

Package: `@ttoon/shared`

## Read T-JSON Batch Text

### `parse<T>(text, options?): T`

- Parses `T-JSON`, `T-TOON`, or `typed_unit` text to JS values
- `ParseOptions`:

| Property | Type | Default | Description |
| :--- | :--- | :--- | :--- |
| `mode` | `'compat' \| 'strict'` | `'compat'` | Parse mode for the T-TOON / typed-unit path |
| `codecs` | `CodecRegistry` | — | Per-call codec overrides |

### `readArrow(text): Promise<ArrowTable>`

- Parses batch text to Arrow Table
- Auto-detects format
- Requires `apache-arrow`

## Write T-JSON Batch Text

### `toTjson(value, options?): string`

- Serializes JS values to T-JSON text

`TjsonSerializeOptions`:

| Property | Type | Default | Description |
| :--- | :--- | :--- | :--- |
| `binaryFormat` | `'hex' \| 'b64'` | `'hex'` | Binary encoding |

### `stringifyArrowTjson(table, options?): Promise<string>`

- Serializes Arrow Table to T-JSON list-of-objects
- Requires `apache-arrow`

## Transcode into T-JSON

### `ttoonToTjson(text, options?): string`

- Converts T-TOON text directly to T-JSON text
- `TtoonToTjsonOptions` extends `TjsonSerializeOptions` and adds `mode?: ParseMode`

Note: for JS direct transcode, `TranscodeError.phase` is currently not reliable. Prefer `sourceKind` and the underlying `source.message`.

## Related Utilities

- `detectFormat(text)` is documented on [Format Detection](./format-detection.md)
- JS type markers such as `toon.uuid()` and `toon.decimal()` apply when you serialize JS values into T-JSON

</TabItem>
<TabItem value="rust" label="Rust">

Crate: `ttoon-core`

## Read T-JSON Batch Text

```rust
fn from_ttoon(text: &str) -> Result<ir::Node>
fn from_ttoon_with_mode(text: &str, mode: ParseMode) -> Result<ir::Node>
fn read_arrow(text: &str) -> Result<ir::ArrowTable>
```

- Despite the historical function name, these APIs auto-detect T-JSON input too
- `read_arrow()` supports arrowable batch text regardless of whether the input is T-TOON or T-JSON

## Write T-JSON Batch Text

```rust
fn to_tjson(node: &ir::Node, opts: Option<&TjsonOptions>) -> Result<String>
fn arrow_to_tjson(table: &ir::ArrowTable, opts: Option<&TjsonOptions>) -> Result<String>
```

## Transcode into T-JSON

```rust
fn ttoon_to_tjson(text: &str, mode: ParseMode, opts: Option<&TjsonOptions>) -> Result<String>
```

## Configuration Types

### `TjsonOptions`

```rust
pub struct TjsonOptions {
    pub binary_format: BinaryFormat,
}
```

### `ParseMode`

```rust
pub enum ParseMode {
    Strict,
    Compat,
}
```

### `BinaryFormat`

```rust
pub enum BinaryFormat { Hex, B64 }
```

## Related Utilities

- `detect_format(input: &str)` is documented on [Format Detection](./format-detection.md)
- Shared schema and streaming types are documented on [Stream API](./stream-api.md)

</TabItem>
</Tabs>

## Related Pages

- **[T-TOON Batch API](./ttoon-batch-api.md)** — Batch APIs centered on T-TOON text
- **[Stream API](./stream-api.md)** — Row-by-row readers and writers
- **[Typed Value Reference](./typed-value-reference.md)** — Value-level semantics shared by both batch syntaxes
