---
title: Stream API
sidebar_position: 5
sidebar_label: Stream API
description: Python、JavaScript 與 Rust 的 T-TOON / T-JSON 串流讀寫 API。
---

# Stream API

本頁整理 `T-TOON` 與 `T-JSON` 的逐行串流 API。可透過上方語言 tabs 在 Python、JavaScript 與 Rust 之間切換。

所有 streaming API 都需要 `StreamSchema`。

共通格式慣例：

- **T-TOON stream**：`[*]{fields}:`
- **T-JSON stream**：最外層為物件陣列
- **Object path**：以語言原生物件逐列處理
- **Arrow path**：以 Arrow batch 逐批處理

import Tabs from '@theme/Tabs';
import TabItem from '@theme/TabItem';

<Tabs groupId="language">
<TabItem value="python" label="Python">

套件：`ttoon`

## Reader Factories

| 函數 | 回傳型別 | 格式 | 路徑 |
| :--- | :--- | :--- | :--- |
| `stream_read(source, *, schema, mode=None, codecs=None)` | `StreamReader` | T-TOON | Object |
| `stream_read_tjson(source, *, schema, mode=None, codecs=None)` | `TjsonStreamReader` | T-JSON | Object |
| `stream_read_arrow(source, *, schema, batch_size=1024, mode=None)` | `ArrowStreamReader` | T-TOON | Arrow |
| `stream_read_arrow_tjson(source, *, schema, batch_size=1024, mode=None)` | `TjsonArrowStreamReader` | T-JSON | Arrow |

所有 reader 都是 Python iterator：

```python
for row in reader:
    print(row)
```

對 T-JSON 串流 reader 而言，`mode` 不會放寬 JSON 值語法；它只控制 schema 外欄位的處理方式：`compat` 會略過，`strict` 會報錯。

## Writer Factories

| 函數 | 回傳型別 | 格式 | 路徑 |
| :--- | :--- | :--- | :--- |
| `stream_writer(sink, *, schema, delimiter=",", binary_format=None, codecs=None)` | `StreamWriter` | T-TOON | Object |
| `stream_writer_tjson(sink, *, schema, binary_format=None, codecs=None)` | `TjsonStreamWriter` | T-JSON | Object |
| `stream_writer_arrow(sink, *, schema, delimiter=",", binary_format=None)` | `ArrowStreamWriter` | T-TOON | Arrow |
| `stream_writer_arrow_tjson(sink, *, schema, binary_format=None)` | `TjsonArrowStreamWriter` | T-JSON | Arrow |

所有 writer 都支援 context manager：

```python
with stream_writer(sink, schema=schema) as writer:
    writer.write({"name": "Alice", "score": 95})
result = writer.result
```

## Writer Methods 與結果

| 類別 | 寫入方法 | 備註 |
| :--- | :--- | :--- |
| `StreamWriter` | `write(row: Mapping)` | 物件列 |
| `TjsonStreamWriter` | `write(row: Mapping)` | 物件列 |
| `ArrowStreamWriter` | `write_batch(batch)` | Arrow `RecordBatch` |
| `TjsonArrowStreamWriter` | `write_batch(batch)` | Arrow `RecordBatch` |

`StreamResult`：

| 屬性 | 型別 | 描述 |
| :--- | :--- | :--- |
| `rows_emitted` | `int` | 已寫入列數 |

## Codec 作用範圍

### `use(codecs) -> None`

為 Python object-path streaming APIs 註冊全域 codec。

Codec 會影響：

- `stream_read()` / `stream_writer()`
- `stream_read_tjson()` / `stream_writer_tjson()`

不影響批次 `loads()`、批次 `to_tjson()`、Arrow-path streaming 與 direct transcode。

</TabItem>
<TabItem value="js" label="JavaScript / TypeScript">

套件：`@ttoon/shared`

## Reader Factories

所有 reader 都回傳 `AsyncIterable`：

| 函數 | 回傳型別 | 格式 | 路徑 |
| :--- | :--- | :--- | :--- |
| `streamRead(source, opts)` | `AsyncIterable<Record<string, unknown>>` | T-TOON | Object |
| `streamReadTjson(source, opts)` | `AsyncIterable<Record<string, unknown>>` | T-JSON | Object |
| `streamReadArrow(source, opts)` | `AsyncIterable<RecordBatch>` | T-TOON | Arrow |
| `streamReadArrowTjson(source, opts)` | `AsyncIterable<RecordBatch>` | T-JSON | Arrow |

`StreamReadOptions`：

| 屬性 | 型別 | 必填 | 描述 |
| :--- | :--- | :--- | :--- |
| `schema` | `StreamSchema \| StreamSchemaInput` | 是 | 欄位定義 |
| `mode` | `ParseMode` | 否 | 解析模式；對 T-JSON 串流來說，主要控制 schema 外欄位的處理方式 |
| `codecs` | `CodecRegistry` | 否 | object reader 的 codec 覆寫 |

`StreamReadArrowOptions`：

| 屬性 | 型別 | 必填 | 描述 |
| :--- | :--- | :--- | :--- |
| `schema` | `StreamSchema \| StreamSchemaInput` | 是 | 欄位定義 |
| `batchSize` | `number` | 否 | 每個 Arrow batch 的列數 |
| `mode` | `ParseMode` | 否 | 解析模式；對 T-JSON 串流來說，主要控制 schema 外欄位的處理方式 |

## Writer Factories

| 函數 | 回傳型別 | 格式 | 路徑 |
| :--- | :--- | :--- | :--- |
| `streamWriter(sink, opts)` | `StreamWriter` | T-TOON | Object |
| `streamWriterTjson(sink, opts)` | `TjsonStreamWriter` | T-JSON | Object |
| `streamWriterArrow(sink, opts)` | `ArrowStreamWriter` | T-TOON | Arrow |
| `streamWriterArrowTjson(sink, opts)` | `TjsonArrowStreamWriter` | T-JSON | Arrow |

Object writer options：

| 屬性 | 型別 | 必填 | 描述 |
| :--- | :--- | :--- | :--- |
| `schema` | `StreamSchema \| StreamSchemaInput` | 是 | 欄位定義 |
| `delimiter` | `',' \| '\t' \| '\|'` | 否 | T-TOON tabular 分隔符 |
| `binaryFormat` | `'hex' \| 'b64'` | 否 | 二進位編碼 |
| `codecs` | `CodecRegistry` | 否 | object writer 的 codec 覆寫 |

Arrow writer 沒有 `codecs`；T-JSON writer 沒有 `delimiter`。

## Writer Classes 與結果

| 類別 | 方法 | 輸入 |
| :--- | :--- | :--- |
| `StreamWriter` | `write(row)` | `Record<string, unknown>` |
| `TjsonStreamWriter` | `write(row)` | `Record<string, unknown>` |
| `ArrowStreamWriter` | `writeBatch(batch)` | `RecordBatch` |
| `TjsonArrowStreamWriter` | `writeBatch(batch)` | `RecordBatch` |

所有 writer 都提供：

- `close(): Promise<StreamResult>`
- `result: StreamResult | undefined`

`StreamResult`：

| 屬性 | 型別 | 描述 |
| :--- | :--- | :--- |
| `rowsEmitted` | `number` | 已寫入列數 |

## Source / Sink Types

- `TextSource`: `string | Iterable<string | Uint8Array> | AsyncIterable<string | Uint8Array> | ReadableStreamLike<string | Uint8Array>`
- `TextSink`: `((chunk: string) => void | Promise<void>) | { write(chunk: string): void | Promise<void> } | WritableStreamLike<string>`

## Codec 作用範圍

### `use(codecs): Promise<void>`

為 JS object-path parse / serialize 註冊全域 codec，其中也包含 object-path 的 stream readers / writers。Arrow-path streaming 是 schema-driven，不使用 codec。

</TabItem>
<TabItem value="rust" label="Rust">

Crate：`ttoon-core`

## Reader Types

| 型別 | 格式 | 輸出 |
| :--- | :--- | :--- |
| `StreamReader` | T-TOON | `IndexMap<String, Node>` |
| `TjsonStreamReader` | T-JSON | `IndexMap<String, Node>` |
| `ArrowStreamReader` | T-TOON | Arrow `RecordBatch` |
| `TjsonArrowStreamReader` | T-JSON | Arrow `RecordBatch` |

範例：

```rust
let reader = StreamReader::new(source, schema);
for row in reader {
    let row = row?;
}
```

其他 reader constructors：

- `StreamReader::with_mode(source, schema, ParseMode)`
- `TjsonStreamReader::new(source, schema)`
- `TjsonStreamReader::with_mode(source, schema, ParseMode)`
- `ArrowStreamReader::new(source, schema, batch_size) -> Result<Self>`
- `ArrowStreamReader::with_mode(source, schema, batch_size, ParseMode) -> Result<Self>`
- `TjsonArrowStreamReader::new(source, schema, batch_size) -> Result<Self>`
- `TjsonArrowStreamReader::with_mode(source, schema, batch_size, ParseMode) -> Result<Self>`

## Writer Types

| 型別 | 格式 | 輸入 |
| :--- | :--- | :--- |
| `StreamWriter` | T-TOON | Row values |
| `TjsonStreamWriter` | T-JSON | Row values |
| `ArrowStreamWriter` | T-TOON | Arrow `RecordBatch` |
| `TjsonArrowStreamWriter` | T-JSON | Arrow `RecordBatch` |

範例：

```rust
let mut writer = StreamWriter::new(output, schema, TtoonOptions::default());
writer.write(&row)?;
let result = writer.close()?;
println!("rows: {}", result.rows_emitted);
```

## Stream Result

```rust
pub struct StreamResult {
    pub rows_emitted: usize,
}
```

## Schema 與設定型別

- `StreamSchema`、`StreamField`、`FieldType`、`ScalarType` 的定義就在本頁下方
- T-TOON stream writer 使用 `TtoonOptions`
- T-JSON stream writer 使用 `TjsonOptions`
- 支援解析模式的 reader 使用 `ParseMode`

</TabItem>
</Tabs>

## Stream Schema

`StreamSchema` 定義串流操作中的欄位名稱與型別。所有串流讀寫器都需要 schema。

### 建立方式

<Tabs groupId="language">
<TabItem value="python" label="Python">

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

</TabItem>
<TabItem value="js" label="JavaScript / TypeScript">

```ts
import { StreamSchema, types } from '@ttoon/shared';

// 從 object 建立
const schema = new StreamSchema({
  name: types.string,
  score: types.int,
  amount: types.decimal(10, 2),
});

// 也接受 iterable input，例如 array-of-tuples
const schemaFromTuples = new StreamSchema([
  ['name', types.string],
  ['score', types.int],
]);
```

</TabItem>
<TabItem value="rust" label="Rust">

```rust
use ttoon_core::{StreamSchema, FieldType, ScalarType};

let schema = StreamSchema::new([
    ("name", FieldType::new(ScalarType::String)),
    ("score", FieldType::new(ScalarType::Int)),
    ("amount", FieldType::new(ScalarType::Decimal { precision: 10, scale: 2 })),
]);

// 可失敗的建構方式
let schema = StreamSchema::try_new(fields)?;
```

</TabItem>
</Tabs>

### Types 命名空間

| 型別 | Python | JavaScript | Rust |
| :--- | :--- | :--- | :--- |
| String | `types.string` | `types.string` | `ScalarType::String` |
| Int | `types.int` | `types.int` | `ScalarType::Int` |
| Float | `types.float` | `types.float` | `ScalarType::Float` |
| Bool | `types.bool` | `types.bool` | `ScalarType::Bool` |
| Date | `types.date` | `types.date` | `ScalarType::Date` |
| Time | `types.time` | `types.time` | `ScalarType::Time` |
| DateTime（帶時區） | `types.datetime` | `types.datetime` | `ScalarType::DateTime { has_tz: true }` |
| DateTime（無時區） | `types.datetime_naive` | `types.datetimeNaive` | `ScalarType::DateTime { has_tz: false }` |
| UUID | `types.uuid` | `types.uuid` | `ScalarType::Uuid` |
| Binary | `types.binary` | `types.binary` | `ScalarType::Binary` |
| Decimal(p, s) | `types.decimal(p, s)` | `types.decimal(p, s)` | `ScalarType::decimal(p, s)` 或 `ScalarType::Decimal { precision, scale }` |

Rust 另提供便利建構子 `ScalarType::datetime()` 與 `ScalarType::datetime_naive()`。

### 可為 Null 的欄位

所有型別規格都支援 `.nullable()` 讓欄位可接受 null：

<Tabs groupId="language">
<TabItem value="python" label="Python">

```python
schema = StreamSchema({
    "name": types.string,                 # NOT NULL
    "nickname": types.string.nullable(),  # nullable
})
```

</TabItem>
<TabItem value="js" label="JavaScript / TypeScript">

```ts
const schema = new StreamSchema({
  name: types.string,
  nickname: types.string.nullable(),
});
```

</TabItem>
<TabItem value="rust" label="Rust">

```rust
StreamSchema::new([
    ("name", FieldType::new(ScalarType::String)),
    ("nickname", FieldType::nullable(ScalarType::String)),
]);
```

</TabItem>
</Tabs>

### Schema 存取

<Tabs groupId="language">
<TabItem value="python" label="Python">

```python
schema["name"]     # 傳回以 ttoon.types 建立的欄位規格
len(schema)        # 欄位數量
list(schema)       # 欄位名稱列表
schema.export()    # 可序列化形式
```

</TabItem>
<TabItem value="js" label="JavaScript / TypeScript">

```ts
schema.get("name")    // FieldTypeSpec | undefined
schema.keys()         // IterableIterator<string>
schema.values()       // IterableIterator<FieldTypeSpec>
schema.entries()      // IterableIterator<[string, FieldTypeSpec]>
schema.export()       // 可序列化形式
```

</TabItem>
<TabItem value="rust" label="Rust">

```rust
schema.field("name")   // Option<&StreamField>
schema.fields()        // &[StreamField]
schema.len()           // usize
schema.is_empty()      // bool
```

</TabItem>
</Tabs>

### 驗證規則

三種語言都遵循相同的概念規則：

- schema 至少要有一個欄位
- 欄位名稱必須是字串
- 不允許重複欄位名稱
- 欄位型別必須來自各語言對應的 typed schema surface

各語言錯誤表現：

- Python：名稱 / 型別不合法時拋 `TypeError`；重複或空 schema 拋 `ValueError`
- JavaScript：名稱 / 型別不合法時拋 `TypeError`；重複或空 schema 拋 `Error`
- Rust：`StreamSchema::try_new()` 回傳 `Result`；`StreamSchema::new()` 在非法輸入時 panic

### Decimal 約束

`decimal(precision, scale)` 會轉交 Rust 後端處理。有效限制如下：

- `precision` 必須介於 `1` 到 `76`
- `scale` 必須符合 Rust `i8`
- Arrow 轉換時，`precision <= 38` 會使用 `Decimal128`，其餘使用 `Decimal256`

超出範圍的值可能在 Python / JS 包裝層被建構成功，但最終仍會在 Rust 驗證或轉換時失敗。

### Arrow Schema 轉換（Rust）

```rust
// StreamSchema -> Arrow Schema
let arrow_schema = schema.to_arrow_schema()?;

// Arrow Schema -> StreamSchema
let stream_schema = StreamSchema::from_arrow_schema(&arrow_schema)?;
```

## 相關頁面

- **[T-TOON 批次 API](./ttoon-batch-api.md)** — 非串流的 T-TOON API
- **[T-JSON 批次 API](./tjson-batch-api.md)** — 非串流的 T-JSON API
