---
title: Rust 指南
sidebar_position: 3
sidebar_label: Rust
description: 使用 Rust 操作 TTOON 的完整指南 — 包含批次處理、Arrow、串流與轉碼。
---

# Rust 指南

`ttoon-core` crate 是標準的 TTOON 引擎。Python 和 JavaScript SDK 都在背後委派這個 crate 執行解析和序列化。

## 安裝

```bash
cargo add ttoon-core
```

## 批次操作 (Batch Operations)

### 反序列化：`from_ttoon()`

```rust
use ttoon_core::{from_ttoon, from_ttoon_with_mode, ParseMode};

// 自動偵測格式，預設使用 compat 模式
let node = from_ttoon("name: \"Alice\"\nage: 30")?;

// 嚴格模式 (Strict mode)
let node = from_ttoon_with_mode("{\"key\": 42}", ParseMode::Strict)?;
```

回傳 `ir::Node` — 內部表示 (internal representation) 樹。

注意：`from_ttoon()` 明確使用了 `ParseMode::Compat`。這與 `ParseMode::default()` 不同，後者是 `Strict`。

### 序列化：`to_ttoon()` / `to_tjson()`

```rust
use ttoon_core::{to_ttoon, to_tjson, BinaryFormat, Delimiter, TjsonOptions, TtoonOptions};

let text = to_ttoon(&node, None)?;

let opts = TtoonOptions {
    binary_format: BinaryFormat::B64,
    indent_size: 4,
    delimiter: Delimiter::Tab,
};
let text = to_ttoon(&node, Some(&opts))?;
let json = to_tjson(&node, None)?;
```

**`TtoonOptions`:**

| 欄位 | 型別 | 預設值 | 描述 |
| :--- | :--- | :--- | :--- |
| `binary_format` | `BinaryFormat` | `Hex` | `Hex` 或 `B64` |
| `indent_size` | `u8` | `2` | 縮排寬度 |
| `delimiter` | `Delimiter` | `Comma` | `Comma`, `Tab` 或 `Pipe` |

### 格式偵測

```rust
use ttoon_core::detect_format;
use ttoon_core::format_detect::Format;

let fmt = detect_format("{\"key\": 42}");
assert_eq!(fmt, Format::Tjson);

let fmt = detect_format("key: 42");
assert_eq!(fmt, Format::TypedUnit);
```

## Arrow 路徑

```rust
use ttoon_core::{read_arrow, arrow_to_ttoon, arrow_to_tjson};

// 文字 → Arrow
let table = read_arrow(text)?;

// Arrow → T-TOON 表格
let ttoon = arrow_to_ttoon(&table, None)?;

// Arrow → T-JSON
let tjson = arrow_to_tjson(&table, None)?;
```

`read_arrow()` 會自動偵測格式。輸入必須是具有純量欄位值的統一物件列表。

## 直接轉碼 (Direct Transcode)

```rust
use ttoon_core::{tjson_to_ttoon, ttoon_to_tjson, ParseMode};

// T-JSON → T-TOON (始終是嚴格解析)
let ttoon = tjson_to_ttoon(r#"{"key": 42}"#, None)?;

// T-TOON → T-JSON (可設定解析模式)
let tjson = ttoon_to_tjson("key: 42", ParseMode::Compat, None)?;
```

## 串流 (Streaming)

### Schema 定義

```rust
use ttoon_core::{StreamSchema, FieldType, ScalarType};

let schema = StreamSchema::new([
    ("name", FieldType::new(ScalarType::String)),
    ("score", FieldType::new(ScalarType::Int)),
    ("amount", FieldType::new(ScalarType::decimal(10, 2))),
    ("active", FieldType::nullable(ScalarType::Bool)),
]);
```

### 串流讀取器 (T-TOON)

```rust
use std::io::Cursor;

use ttoon_core::{ParseMode, StreamReader};

let text = "[1]{name,score,amount,active}:\n\"Alice\", 95, 123.45m, true\n";
let reader = StreamReader::new(Cursor::new(text), schema.clone());
let strict_reader = StreamReader::with_mode(Cursor::new(text), schema, ParseMode::Strict);
for row in reader {
    let row = row?;
    // row 為 IndexMap<String, Node>
}
```

### 串流寫入器 (T-TOON)

```rust
use std::io::Cursor;

use ttoon_core::{StreamReader, StreamWriter, TtoonOptions};

let text = "[1]{name,score,amount,active}:\n\"Alice\", 95, 123.45m, true\n";
let row = StreamReader::new(Cursor::new(text), schema.clone())
    .next()
    .transpose()?
    .unwrap();
let mut writer = StreamWriter::new(Vec::new(), schema, TtoonOptions::default());
writer.write(&row)?;
let result = writer.close()?;
println!("輸出的資料列數: {}", result.rows_emitted);
```

### T-JSON 串流

```rust
use std::io::Cursor;

use ttoon_core::{TjsonOptions, TjsonStreamReader, TjsonStreamWriter};

// 模式與上述相同，但適用於 T-JSON 格式
let reader = TjsonStreamReader::new(Cursor::new(r#"[{"name": "Alice"}]"#), schema.clone());
let mut writer = TjsonStreamWriter::new(Vec::new(), schema, TjsonOptions::default());
```

### Arrow 串流

```rust
use std::io::Cursor;

use ttoon_core::{ArrowStreamReader, ArrowStreamWriter, ParseMode, TtoonOptions, read_arrow};

let text = "[1]{name,score,amount,active}:\n\"Alice\", 95, 123.45m, true\n";
let reader = ArrowStreamReader::new(Cursor::new(text), schema.clone(), batch_size)?;
let strict_reader =
    ArrowStreamReader::with_mode(Cursor::new(text), schema.clone(), batch_size, ParseMode::Strict)?;
for batch in reader {
    let batch = batch?;  // Arrow RecordBatch
}

let batch = read_arrow(text)?.batches.into_iter().next().unwrap();
let mut writer = ArrowStreamWriter::new(Vec::new(), schema, TtoonOptions::default())?;
writer.write_batch(&batch)?;
let result = writer.close()?;
```

共有 8 種串流變體可用：`StreamReader`, `StreamWriter`, `TjsonStreamReader`, `TjsonStreamWriter`, `ArrowStreamReader`, `ArrowStreamWriter`, `TjsonArrowStreamReader`, `TjsonArrowStreamWriter`。

## 錯誤處理

```rust
use ttoon_core::{ErrorKind, from_ttoon};

match from_ttoon(text) {
    Ok(node) => { /* ... */ }
    Err(e) => {
        match e.kind {
            ErrorKind::ParseError => {
                if let Some(span) = e.span {
                    println!("解析錯誤於 {}:{}: {}", span.line, span.column, e.message);
                } else {
                    println!("解析錯誤: {}", e.message);
                }
            }
            ErrorKind::ArrowError => println!("Arrow 錯誤: {}", e.message),
            _ => println!("錯誤: {:?}", e),
        }
    }
}
```

## 下一步

- **[Arrow 與 Polars 指南](arrow-and-polars.md)** — 表格路徑詳細資訊
- **[串流指南](streaming.md)** — 逐行處理模式
- **[Rust API 參考資料](../reference/rust-api.md)** — 完整的 API 簽名
