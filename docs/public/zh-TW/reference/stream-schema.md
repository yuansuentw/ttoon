---
title: 串流 Schema (Stream Schema)
sidebar_position: 8
sidebar_label: 串流 Schema
description: 用於跨語言定義具型別欄位 schemas 的 StreamSchema API 參考。
---

# 串流 Schema (Stream Schema)

`StreamSchema` 定義了串流操作的欄位名稱與型別。所有串流讀寫器都需要 Schema。

## 建立 (Construction)

### Python

```python
from ttoon import StreamSchema, types

# 從 dict 建立
schema = StreamSchema({
    "name": types.string,
    "score": types.int,
    "amount": types.decimal(10, 2),
})

# 從 list of tuples 建立（保留插入順序）
schema = StreamSchema([
    ("name", types.string),
    ("score", types.int),
])
```

### JavaScript / TypeScript

```ts
import { StreamSchema, types } from '@ttoon/shared';

// 從 object 建立
const schema = new StreamSchema({
  name: types.string,
  score: types.int,
  amount: types.decimal(10, 2),
});

// 也接受可疊代輸入，例如 array-of-tuples
const schemaFromTuples = new StreamSchema([
  ['name', types.string],
  ['score', types.int],
]);
```

### Rust

```rust
use ttoon_core::{StreamSchema, FieldType, ScalarType};

let schema = StreamSchema::new([
    ("name", FieldType::new(ScalarType::String)),
    ("score", FieldType::new(ScalarType::Int)),
    ("amount", FieldType::new(ScalarType::Decimal { precision: 10, scale: 2 })),
]);

// 可能失敗的建構方式
let schema = StreamSchema::try_new(fields)?;
```

## `Types` 命名空間 (Namespace)

| 型別 | Python | JavaScript | Rust |
| :--- | :--- | :--- | :--- |
| String | `types.string` | `types.string` | `ScalarType::String` |
| Int | `types.int` | `types.int` | `ScalarType::Int` |
| Float | `types.float` | `types.float` | `ScalarType::Float` |
| Bool | `types.bool` | `types.bool` | `ScalarType::Bool` |
| Date | `types.date` | `types.date` | `ScalarType::Date` |
| Time | `types.time` | `types.time` | `ScalarType::Time` |
| DateTime (帶時區) | `types.datetime` | `types.datetime` | `ScalarType::DateTime { has_tz: true }` |
| DateTime (無時區) | `types.datetime_naive` | `types.datetimeNaive` | `ScalarType::DateTime { has_tz: false }` |
| UUID | `types.uuid` | `types.uuid` | `ScalarType::Uuid` |
| Binary | `types.binary` | `types.binary` | `ScalarType::Binary` |
| Decimal(p, s) | `types.decimal(p, s)` | `types.decimal(p, s)` | `ScalarType::decimal(p, s)` 或 `ScalarType::Decimal { precision, scale }` |

Rust 另提供便利建構子 `ScalarType::datetime()` 和 `ScalarType::datetime_naive()`。

## 可為 Null 的欄位 (Nullable Fields)

所有型別規格都支援 `.nullable()` 以允許欄位接受 Null：

```python
schema = StreamSchema({
    "name": types.string,                 # NOT NULL
    "nickname": types.string.nullable(),  # nullable
})
```

```ts
const schema = new StreamSchema({
  name: types.string,
  nickname: types.string.nullable(),
});
```

```rust
StreamSchema::new([
    ("name", FieldType::new(ScalarType::String)),
    ("nickname", FieldType::nullable(ScalarType::String)),
]);
```

## Schema 存取

### Python

```python
schema["name"]     # 傳回 ttoon.types 的欄位規格實例
len(schema)        # 欄位數量
list(schema)       # 所有欄位名稱
schema.export()    # 可序列化的形式
```

### JavaScript

```ts
schema.get("name")    // FieldTypeSpec | undefined
schema.keys()         // IterableIterator<string>
schema.values()       // IterableIterator<FieldTypeSpec>
schema.entries()      // IterableIterator<[string, FieldTypeSpec]>
schema.export()       // 可序列化的形式
```

### Rust

```rust
schema.field("name")   // Option<&StreamField>
schema.fields()        // &[StreamField]
schema.len()           // usize
schema.is_empty()      // bool
```

## 驗證規則 (Validation Rules)

三種語言皆遵循相同的概念規則：

- Schema 至少須包含一個欄位
- 欄位名稱必須為字串
- 不允許重複的欄位名稱
- 欄位型別必須來自各語言提供的 typed schema 介面

各語言的錯誤表現：

- Python：無效的名稱/型別引發 `TypeError`；重複或空 Schema 引發 `ValueError`
- JavaScript：無效的名稱/型別引發 `TypeError`；重複或空 Schema 引發 `Error`
- Rust：`StreamSchema::try_new()` 回傳 `Result`；`StreamSchema::new()` 在輸入不合法時 panic

## Decimal 約束

`decimal(precision, scale)` 會轉交至 Rust 後端處理。有效後端限制：

- `precision` 須在 `1` 至 `76` 之間
- `scale` 須符合 Rust `i8` 範圍
- Arrow 轉換時，`precision <= 38` 使用 `Decimal128`，其餘使用 `Decimal256`

超出範圍的值可能在 Python/JS 包裝層建構時被接受，但最終在 Rust 驗證或轉換時會失敗。

## Arrow Schema 轉換（僅限 Rust）

```rust
// StreamSchema → Arrow Schema
let arrow_schema = schema.to_arrow_schema()?;

// Arrow Schema → StreamSchema
let stream_schema = StreamSchema::from_arrow_schema(&arrow_schema)?;
```

## 與串流的關係

StreamSchema 為所有串流操作的必要條件：

- **讀取器**：Schema 定義了預期的欄位名稱與型別
- **寫入器**：Schema 定義了輸出標頭與值的序列化規則
- **T-TOON 串流**：Schema 對應至 `[*]{fields}:` 標頭
- **T-JSON 串流**：Schema 定義了每個 JSON 物件中預期的鍵
