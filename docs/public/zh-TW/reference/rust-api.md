---
title: Rust API 參考資料 (Rust API Reference)
sidebar_position: 4
sidebar_label: Rust API
description: ttoon-core crate 的完整 Rust API 參考資料。
---

# Rust API 參考資料 (Rust API Reference)

Crate: `ttoon-core` (crates.io)

## 批次 API (Batch APIs)

### 反序列化 (Deserialization)

```rust
fn from_ttoon(text: &str) -> Result<ir::Node>
fn from_ttoon_with_mode(text: &str, mode: ParseMode) -> Result<ir::Node>
fn read_arrow(text: &str) -> Result<ir::ArrowTable>
```

- `from_ttoon()` 自動偵測格式，預設使用 `ParseMode::Compat`
- `ParseMode::default()` 為 `ParseMode::Strict`；`from_ttoon()` 則明確選擇 `Compat`
- `read_arrow()` 自動偵測格式，輸入必須為統一物件列表 (list-of-uniform-objects)

### 序列化 (Serialization)

```rust
fn to_ttoon(node: &ir::Node, opts: Option<&TtoonOptions>) -> Result<String>
fn to_tjson(node: &ir::Node, opts: Option<&TjsonOptions>) -> Result<String>
fn arrow_to_ttoon(table: &ir::ArrowTable, opts: Option<&TtoonOptions>) -> Result<String>
fn arrow_to_tjson(table: &ir::ArrowTable, opts: Option<&TjsonOptions>) -> Result<String>
```

### 格式偵測 (Format Detection)

```rust
fn detect_format(input: &str) -> format_detect::Format
```

回傳 `format_detect::Format::Tjson`、`format_detect::Format::Ttoon` 或 `format_detect::Format::TypedUnit`。

## 轉碼 API (Transcode APIs)

```rust
fn tjson_to_ttoon(text: &str, opts: Option<&TtoonOptions>) -> Result<String>
fn ttoon_to_tjson(text: &str, mode: ParseMode, opts: Option<&TjsonOptions>) -> Result<String>
```

- `tjson_to_ttoon()` 一律使用嚴格的 T-JSON 解析
- `ttoon_to_tjson()` 接受可配置的 `ParseMode` 參數

## 設定型別 (Configuration Types)

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
    Strict,  // 未知的 bare token → 報錯
    Compat,  // 未知的 bare token → 視為字串
}
```

`ParseMode::default()` 回傳 `ParseMode::Strict`。

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
// 建構
StreamSchema::new(fields)           // 輸入不合法時 panic
StreamSchema::try_new(fields)       // 回傳 Result

// 與 Arrow 互轉
StreamSchema::from_arrow_schema(schema) -> Result<Self>
StreamSchema::to_arrow_schema(&self) -> Result<ArrowSchema>

// 存取
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
FieldType::new(scalar_type: ScalarType) -> Self          // 不可為 Null
FieldType::nullable(scalar_type: ScalarType) -> Self     // 可為 Null
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

`null` 值透過 `FieldType::nullable(...)` 表示，而非作為獨立的 `ScalarType` 變體。

便利建構子：

```rust
ScalarType::decimal(10, 2)
ScalarType::datetime()
ScalarType::datetime_naive()
```

## 串流 API (Streaming APIs)

所有 8 種串流型別遵循相同的操作模式：

### 讀取器 (Readers)

| 型別 | 格式 | 輸出 |
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

其他讀取器建構子：

- `StreamReader::with_mode(source, schema, ParseMode)`
- `TjsonStreamReader::new(source, schema)`
- `TjsonStreamReader::with_mode(source, schema, ParseMode)`
- `ArrowStreamReader::new(source, schema, batch_size) -> Result<Self>`
- `ArrowStreamReader::with_mode(source, schema, batch_size, ParseMode) -> Result<Self>`
- `TjsonArrowStreamReader::new(source, schema, batch_size) -> Result<Self>`
- `TjsonArrowStreamReader::with_mode(source, schema, batch_size, ParseMode) -> Result<Self>`

### 寫入器 (Writers)

| 型別 | 格式 | 輸入 |
| :--- | :--- | :--- |
| `StreamWriter` | T-TOON | Row values |
| `TjsonStreamWriter` | T-JSON | Row values |
| `ArrowStreamWriter` | T-TOON | Arrow RecordBatch |
| `TjsonArrowStreamWriter` | T-JSON | Arrow RecordBatch |

```rust
// 建構
let mut writer = StreamWriter::new(output, schema, TtoonOptions::default());

// 寫入
writer.write(&row)?;

// 完成
let result = writer.close()?;
println!("rows: {}", result.rows_emitted);
```

### `StreamResult`

```rust
pub struct StreamResult {
    pub rows_emitted: usize,
}
```

## 錯誤型別 (Error Types)

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
