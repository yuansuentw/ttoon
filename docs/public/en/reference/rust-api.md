---
title: Rust API Reference
sidebar_position: 4
sidebar_label: Rust API
description: Complete Rust API reference for the ttoon-core crate.
---

# Rust API Reference

Crate: `ttoon-core` (crates.io)

## Batch APIs

### Deserialization

```rust
fn from_ttoon(text: &str) -> Result<ir::Node>
fn from_ttoon_with_mode(text: &str, mode: ParseMode) -> Result<ir::Node>
fn read_arrow(text: &str) -> Result<ir::ArrowTable>
```

- `from_ttoon()` auto-detects format, defaults to `ParseMode::Compat`
- `ParseMode::default()` is `ParseMode::Strict`; `from_ttoon()` chooses `Compat` explicitly
- `read_arrow()` auto-detects format, input must be list-of-uniform-objects

### Serialization

```rust
fn to_ttoon(node: &ir::Node, opts: Option<&TtoonOptions>) -> Result<String>
fn to_tjson(node: &ir::Node, opts: Option<&TjsonOptions>) -> Result<String>
fn arrow_to_ttoon(table: &ir::ArrowTable, opts: Option<&TtoonOptions>) -> Result<String>
fn arrow_to_tjson(table: &ir::ArrowTable, opts: Option<&TjsonOptions>) -> Result<String>
```

### Format Detection

```rust
fn detect_format(input: &str) -> format_detect::Format
```

Returns `format_detect::Format::Tjson`, `format_detect::Format::Ttoon`, or `format_detect::Format::TypedUnit`.

## Transcode APIs

```rust
fn tjson_to_ttoon(text: &str, opts: Option<&TtoonOptions>) -> Result<String>
fn ttoon_to_tjson(text: &str, mode: ParseMode, opts: Option<&TjsonOptions>) -> Result<String>
```

- `tjson_to_ttoon()` always uses strict T-JSON parse
- `ttoon_to_tjson()` accepts configurable `ParseMode`

## Configuration Types

### `TtoonOptions` / `TjsonOptions`

```rust
pub struct TtoonOptions {
    pub binary_format: BinaryFormat,  // Hex | B64
    pub indent_size: u8,             // default: 2
    pub delimiter: Delimiter,         // Comma | Tab | Pipe
}

pub struct TjsonOptions {
    pub binary_format: BinaryFormat,  // Hex | B64
}
```

### `ParseMode`

```rust
pub enum ParseMode {
    Strict,  // unknown bare tokens → error
    Compat,  // unknown bare tokens → string
}
```

`ParseMode::default()` returns `ParseMode::Strict`.

### `BinaryFormat`

```rust
pub enum BinaryFormat {
    Hex,
    B64,
}
```

```rust
BinaryFormat::parse("hex") -> Option<BinaryFormat>
```

### `Delimiter`

```rust
pub enum Delimiter {
    Comma,
    Tab,
    Pipe,
}
```

```rust
Delimiter::parse(",") -> Option<Delimiter>
```

## Schema API

### `StreamSchema`

```rust
// Construction
StreamSchema::new(fields)           // panics on invalid input
StreamSchema::try_new(fields)       // returns Result

// From/to Arrow
StreamSchema::from_arrow_schema(schema) -> Result<Self>
StreamSchema::to_arrow_schema(&self) -> Result<ArrowSchema>

// Access
schema.len() -> usize
schema.is_empty() -> bool
schema.fields() -> &[StreamField]
schema.field(name) -> Option<&StreamField>
```

### `StreamField`

```rust
pub struct StreamField {
    name: String,
    field_type: FieldType,
}
```

```rust
stream_field.name() -> &str
stream_field.field_type() -> &FieldType
```

### `FieldType`

```rust
FieldType::new(scalar_type: ScalarType) -> Self          // non-nullable
FieldType::nullable(scalar_type: ScalarType) -> Self     // nullable
field_type.scalar_type() -> &ScalarType
field_type.is_nullable() -> bool
```

### `ScalarType`

```rust
pub enum ScalarType {
    String,
    Int,
    Float,
    Bool,
    Decimal { precision: u8, scale: i8 },
    Date,
    Time,
    DateTime { has_tz: bool },
    Uuid, Binary,
}
```

`null` values are represented through `FieldType::nullable(...)`, not as a standalone `ScalarType` variant.

Convenience constructors:

```rust
ScalarType::decimal(10, 2)
ScalarType::datetime()
ScalarType::datetime_naive()
```

## Streaming APIs

All 8 streaming types follow the same pattern:

### Readers

| Type | Format | Output |
| :--- | :--- | :--- |
| `StreamReader` | T-TOON | `IndexMap<String, Node>` |
| `TjsonStreamReader` | T-JSON | `IndexMap<String, Node>` |
| `ArrowStreamReader` | T-TOON | Arrow RecordBatch |
| `TjsonArrowStreamReader` | T-JSON | Arrow RecordBatch |

```rust
let reader = StreamReader::new(source, schema);
for row in reader {
    let row = row?;
}
```

Other reader constructors:

- `StreamReader::with_mode(source, schema, ParseMode)`
- `TjsonStreamReader::new(source, schema)`
- `TjsonStreamReader::with_mode(source, schema, ParseMode)`
- `ArrowStreamReader::new(source, schema, batch_size) -> Result<Self>`
- `ArrowStreamReader::with_mode(source, schema, batch_size, ParseMode) -> Result<Self>`
- `TjsonArrowStreamReader::new(source, schema, batch_size) -> Result<Self>`
- `TjsonArrowStreamReader::with_mode(source, schema, batch_size, ParseMode) -> Result<Self>`

### Writers

| Type | Format | Input |
| :--- | :--- | :--- |
| `StreamWriter` | T-TOON | Row values |
| `TjsonStreamWriter` | T-JSON | Row values |
| `ArrowStreamWriter` | T-TOON | Arrow RecordBatch |
| `TjsonArrowStreamWriter` | T-JSON | Arrow RecordBatch |

```rust
// Construction
let mut writer = StreamWriter::new(output, schema, TtoonOptions::default());

// Writing
writer.write(&row)?;

// Finish
let result = writer.close()?;
println!("rows: {}", result.rows_emitted);
```

### `StreamResult`

```rust
pub struct StreamResult {
    pub rows_emitted: usize,
}
```

## Error Types

### `Error`

```rust
pub struct Error {
    pub kind: ErrorKind,
    pub message: String,
    pub span: Option<Span>,
    pub transcode: Option<TranscodeError>,
}
```

```rust
Error::new(kind, message, span) -> Error
Error::transcode(operation, phase, source) -> Error
```

### `Span`

```rust
pub struct Span {
    pub offset: usize,
    pub line: usize,
    pub column: usize,
}
```

### `ErrorKind`

```rust
pub enum ErrorKind {
    LexError,
    ParseError,
    ArrowError,
    SerializeError,
    TranscodeError,
}
```

```rust
ErrorKind::as_str() -> &'static str
```

### `TranscodeError`

```rust
pub struct TranscodeError {
    pub operation: TranscodeOperation,  // TjsonToTtoon | TtoonToTjson
    pub phase: TranscodePhase,          // Parse | Serialize
    pub source_kind: ErrorKind,
    pub source: Box<Error>,
}
```
