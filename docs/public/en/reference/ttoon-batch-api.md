---
title: T-TOON Batch API
sidebar_position: 3
sidebar_label: T-TOON Batch API
description: Batch APIs for reading, writing, and transcoding T-TOON across Python, JavaScript, and Rust.
---

# T-TOON Batch API

This page groups the non-streaming APIs that produce or consume T-TOON text. Use the language tabs to switch between Python, JavaScript, and Rust.

Batch parse APIs still auto-detect `T-TOON`, `T-JSON`, and `typed_unit` input. The functions listed here are grouped by their T-TOON use case, not by exclusive parser support.

import Tabs from '@theme/Tabs';
import TabItem from '@theme/TabItem';

<Tabs groupId="language">
<TabItem value="python" label="Python">

Package: `ttoon`

The current Python package depends on `pyarrow>=23.0.0` and `polars>=1.37.1`.

## Read T-TOON Batch Text

### `loads(text, mode=None) -> object`

- Parses `T-TOON`, `T-JSON`, or `typed_unit` text to Python native objects
- `mode`: `"compat"` (default) or `"strict"`
- `mode` only affects the T-TOON parse path

### `read_arrow(text) -> pyarrow.Table`

- Parses batch text directly to `pyarrow.Table`
- Auto-detects input format
- Input must be a list of uniform objects with scalar fields

## Write T-TOON Batch Text

### `dumps(obj, delimiter=",", indent_size=None, binary_format=None) -> str`

- Serializes Python native objects to T-TOON text
- Also accepts `pyarrow.Table`, `pyarrow.RecordBatch`, and `polars.DataFrame`
- Arrow / Polars input routes to the Arrow path automatically
- Uniform object lists output as tabular `[N]{fields}:`

## Transcode into T-TOON

### `tjson_to_ttoon(text, *, delimiter=",", indent_size=None, binary_format=None) -> str`

- Converts T-JSON text directly to T-TOON through Rust IR only
- Always uses strict T-JSON parsing
- Does not accept a `mode` parameter

## T-TOON Batch Options

| Parameter | APIs | Values | Default |
| :--- | :--- | :--- | :--- |
| `delimiter` | `dumps`, `tjson_to_ttoon` | `","`, `"\t"`, `"|"` | `","` |
| `indent_size` | `dumps`, `tjson_to_ttoon` | `int \| None` | `None` |
| `binary_format` | serialize / transcode APIs above | `"hex"`, `"b64"` | `"hex"` |
| `mode` | `loads` | `"compat"`, `"strict"` | `"compat"` |

## Related Utilities

- `detect_format(text) -> str` is documented on [Format Detection](./format-detection.md)
- Python codec registration does not affect `loads()` or batch transcode; it is mainly relevant to streaming APIs

</TabItem>
<TabItem value="js" label="JavaScript / TypeScript">

Package: `@ttoon/shared`

## Read T-TOON Batch Text

### `parse<T>(text, options?): T`

- Parses `T-TOON`, `T-JSON`, or `typed_unit` text to JS values
- `ParseOptions`:

| Property | Type | Default | Description |
| :--- | :--- | :--- | :--- |
| `mode` | `'compat' \| 'strict'` | `'compat'` | Parse mode for the T-TOON / typed-unit path |
| `codecs` | `CodecRegistry` | — | Per-call codec overrides |

### `readArrow(text): Promise<ArrowTable>`

- Parses batch text to Arrow Table
- Auto-detects format
- Requires the optional peer dependency `apache-arrow`

## Write T-TOON Batch Text

### `stringify(value, options?): string`

- Serializes JS values to T-TOON text
- Uniform object lists auto-output as `[N]{fields}:`

`SerializeOptions`:

| Property | Type | Default | Description |
| :--- | :--- | :--- | :--- |
| `indentSize` | `number` | `2` | Indentation width |
| `delimiter` | `',' \| '\t' \| '\|'` | `','` | Tabular separator |
| `binaryFormat` | `'hex' \| 'b64'` | `'hex'` | Binary encoding |

### `stringifyArrow(table, options?): Promise<string>`

- Serializes Arrow Table to T-TOON tabular text
- Requires `apache-arrow`

### JS Type Markers

When serializing non-native JS types to T-TOON, use the `toon` helpers:

```ts
import { toon } from '@ttoon/shared';

toon.uuid('550e8400-e29b-41d4-a716-446655440000');
toon.decimal('123.45');
toon.date('2026-03-08');
toon.time('14:30:00');
toon.datetime('2026-03-08T14:30:00+08:00');
```

## Transcode into T-TOON

### `tjsonToTtoon(text, options?): string`

- Converts T-JSON text directly to T-TOON text
- `TjsonToTtoonOptions` extends `SerializeOptions`

## Related Utilities

- `detectFormat(text)` is documented on [Format Detection](./format-detection.md)
- Arrow helpers and stream schema types are documented on [Stream API](./stream-api.md)

</TabItem>
<TabItem value="rust" label="Rust">

Crate: `ttoon-core`

## Read T-TOON Batch Text

```rust
fn from_ttoon(text: &str) -> Result<ir::Node>
fn from_ttoon_with_mode(text: &str, mode: ParseMode) -> Result<ir::Node>
fn read_arrow(text: &str) -> Result<ir::ArrowTable>
```

- `from_ttoon()` auto-detects format and explicitly uses `ParseMode::Compat`
- `ParseMode::default()` itself is `ParseMode::Strict`
- `read_arrow()` auto-detects format; input must be a list of uniform objects

## Write T-TOON Batch Text

```rust
fn to_ttoon(node: &ir::Node, opts: Option<&TtoonOptions>) -> Result<String>
fn arrow_to_ttoon(table: &ir::ArrowTable, opts: Option<&TtoonOptions>) -> Result<String>
```

## Transcode into T-TOON

```rust
fn tjson_to_ttoon(text: &str, opts: Option<&TtoonOptions>) -> Result<String>
```

- Always uses strict T-JSON parsing

## Configuration Types

### `TtoonOptions`

```rust
pub struct TtoonOptions {
    pub binary_format: BinaryFormat,
    pub indent_size: u8,
    pub delimiter: Delimiter,
}
```

### `ParseMode`

```rust
pub enum ParseMode {
    Strict,
    Compat,
}
```

### `BinaryFormat` / `Delimiter`

```rust
pub enum BinaryFormat { Hex, B64 }
pub enum Delimiter { Comma, Tab, Pipe }
```

- `BinaryFormat::parse("hex") -> Option<BinaryFormat>`
- `Delimiter::parse(",") -> Option<Delimiter>`

## Related Utilities

- `detect_format(input: &str)` is documented on [Format Detection](./format-detection.md)
- Streaming and schema types are documented on [Stream API](./stream-api.md)

</TabItem>
</Tabs>

## Related Pages

- **[T-JSON Batch API](./tjson-batch-api.md)** — Batch APIs centered on T-JSON text
- **[Stream API](./stream-api.md)** — Row-by-row readers and writers
- **[Format Detection](./format-detection.md)** — Auto-detection rules for `ttoon`, `tjson`, and `typed_unit`
