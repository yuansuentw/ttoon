---
title: Rust Guide
sidebar_position: 3
sidebar_label: Rust
description: Complete guide to using TTOON with Rust — batch, Arrow, streaming, and transcode.
---

# Rust Guide

The `ttoon-core` crate is the canonical TTOON engine. Both the Python and JavaScript SDKs delegate to this crate for parsing and serialization.

## Installation

```bash
cargo add ttoon-core
```

## Batch Operations

### Deserialize: `from_ttoon()`

```rust
use ttoon_core::{from_ttoon, from_ttoon_with_mode, ParseMode};

// Auto-detect format, default compat mode
let node = from_ttoon("name: \"Alice\"\nage: 30")?;

// Strict mode
let node = from_ttoon_with_mode("{\"key\": 42}", ParseMode::Strict)?;
```

Returns `ir::Node` — the internal representation tree.

Note: `from_ttoon()` explicitly uses `ParseMode::Compat`. This is separate from `ParseMode::default()`, which is `Strict`.

### Serialize: `to_ttoon()` / `to_tjson()`

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

| Field | Type | Default | Description |
| :--- | :--- | :--- | :--- |
| `binary_format` | `BinaryFormat` | `Hex` | `Hex` or `B64` |
| `indent_size` | `u8` | `2` | Indentation width |
| `delimiter` | `Delimiter` | `Comma` | `Comma`, `Tab`, or `Pipe` |

### Format Detection

```rust
use ttoon_core::detect_format;
use ttoon_core::format_detect::Format;

let fmt = detect_format("{\"key\": 42}");
assert_eq!(fmt, Format::Tjson);

let fmt = detect_format("key: 42");
assert_eq!(fmt, Format::TypedUnit);
```

## Arrow Path

```rust
use ttoon_core::{read_arrow, arrow_to_ttoon, arrow_to_tjson};

// Text → Arrow
let table = read_arrow(text)?;

// Arrow → T-TOON tabular
let ttoon = arrow_to_ttoon(&table, None)?;

// Arrow → T-JSON
let tjson = arrow_to_tjson(&table, None)?;
```

`read_arrow()` auto-detects format. Input must be a list of uniform objects with scalar field values.

## Direct Transcode

```rust
use ttoon_core::{tjson_to_ttoon, ttoon_to_tjson, ParseMode};

// T-JSON → T-TOON (always strict parse)
let ttoon = tjson_to_ttoon(r#"{"key": 42}"#, None)?;

// T-TOON → T-JSON (configurable parse mode)
let tjson = ttoon_to_tjson("key: 42", ParseMode::Compat, None)?;
```

## Streaming

### Schema Definition

```rust
use ttoon_core::{StreamSchema, FieldType, ScalarType};

let schema = StreamSchema::new([
    ("name", FieldType::new(ScalarType::String)),
    ("score", FieldType::new(ScalarType::Int)),
    ("amount", FieldType::new(ScalarType::decimal(10, 2))),
    ("active", FieldType::nullable(ScalarType::Bool)),
]);
```

### Stream Reader (T-TOON)

```rust
use std::io::Cursor;

use ttoon_core::{ParseMode, StreamReader};

let text = "[1]{name,score,amount,active}:\n\"Alice\", 95, 123.45m, true\n";
let reader = StreamReader::new(Cursor::new(text), schema.clone());
let strict_reader = StreamReader::with_mode(Cursor::new(text), schema, ParseMode::Strict);
for row in reader {
    let row = row?;
    // row is IndexMap<String, Node>
}
```

### Stream Writer (T-TOON)

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
println!("rows emitted: {}", result.rows_emitted);
```

### T-JSON Streaming

```rust
use std::io::Cursor;

use ttoon_core::{TjsonOptions, TjsonStreamReader, TjsonStreamWriter};

// Same pattern as above, but for T-JSON format
let reader = TjsonStreamReader::new(Cursor::new(r#"[{"name": "Alice"}]"#), schema.clone());
let mut writer = TjsonStreamWriter::new(Vec::new(), schema, TjsonOptions::default());
```

### Arrow Streaming

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

All 8 streaming variants are available: `StreamReader`, `StreamWriter`, `TjsonStreamReader`, `TjsonStreamWriter`, `ArrowStreamReader`, `ArrowStreamWriter`, `TjsonArrowStreamReader`, `TjsonArrowStreamWriter`.

## Error Handling

```rust
use ttoon_core::{ErrorKind, from_ttoon};

match from_ttoon(text) {
    Ok(node) => { /* ... */ }
    Err(e) => {
        match e.kind {
            ErrorKind::ParseError => {
                if let Some(span) = e.span {
                    println!("Parse error at {}:{}: {}", span.line, span.column, e.message);
                } else {
                    println!("Parse error: {}", e.message);
                }
            }
            ErrorKind::ArrowError => println!("Arrow error: {}", e.message),
            _ => println!("Error: {:?}", e),
        }
    }
}
```

## Next Steps

- **[Arrow & Polars Guide](arrow-and-polars.md)** — Tabular path details
- **[Streaming Guide](streaming.md)** — Row-by-row processing patterns
- **[API Matrix](../reference/api-matrix.md)** — Entry point to the grouped batch and stream API references
