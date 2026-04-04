---
title: 串流指南
sidebar_position: 6
sidebar_label: 串流
description: 用於 T-TOON 和 T-JSON 的逐行串流讀取器與寫入器，提供物件和 Arrow 變體。
---

# 串流指南

TTOON 在兩種格式 (T-TOON, T-JSON) 和兩條路徑 (物件, Arrow) 之間提供了 8 種串流讀寫器組合。所有串流操作都需要定義欄位名稱和型別的 `StreamSchema`。

## 總覽

| | T-TOON 物件 | T-TOON Arrow | T-JSON 物件 | T-JSON Arrow |
| :--- | :--- | :--- | :--- | :--- |
| **讀取器 (Reader)** | `StreamReader` | `ArrowStreamReader` | `TjsonStreamReader` | `TjsonArrowStreamReader` |
| **寫入器 (Writer)** | `StreamWriter` | `ArrowStreamWriter` | `TjsonStreamWriter` | `TjsonArrowStreamWriter` |

## Schema 定義

所有的串流操作都始於 `StreamSchema`：

### Python

```python
from ttoon import StreamSchema, types

schema = StreamSchema({
    "name": types.string,
    "score": types.int,
    "amount": types.decimal(10, 2),
    "active": types.bool.nullable(),
})
```

### JavaScript / TypeScript

```ts
import { StreamSchema, types } from '@ttoon/shared';

const schema = new StreamSchema({
  name: types.string,
  score: types.int,
  amount: types.decimal(10, 2),
  active: types.bool.nullable(),
});
```

### Rust

```rust
use ttoon_core::{StreamSchema, FieldType, ScalarType};

let schema = StreamSchema::new([
    ("name", FieldType::new(ScalarType::String)),
    ("score", FieldType::new(ScalarType::Int)),
    ("amount", FieldType::new(ScalarType::Decimal { precision: 10, scale: 2 })),
    ("active", FieldType::nullable(ScalarType::Bool)),
]);
```

### 可用型別

| 型別規格 | Python | JavaScript |描述 |
| :--- | :--- | :--- | :--- |
| 字串 (String) | `types.string` | `types.string` |字串 |
| 整數 (Int) | `types.int` | `types.int` |整數 |
| 浮點數 (Float)| `types.float` | `types.float` |浮點數 |
| 布林 (Bool) | `types.bool` | `types.bool` |布林值 |
| 日期 (Date) | `types.date` | `types.date` |日期 |
| 時間 (Time) | `types.time` | `types.time` |時間 |
| 日期時間 (DateTime)| `types.datetime` | `types.datetime` |帶時區的日期時間 |
| 無時區日期時間 (DateTime Naive) | `types.datetime_naive` | `types.datetimeNaive` |不帶時區的日期時間 |
| UUID | `types.uuid` | `types.uuid` |UUID |
| 二進位 (Binary)| `types.binary` | `types.binary` |二進位 |
| 十進位 (Decimal)| `types.decimal(p, s)` | `types.decimal(p, s)` |Decimal(精度 precision, 小數 scale)|

所有型別都支援 `.nullable()` 來允許 null 值。

## T-TOON 串流

### 寫入

T-TOON 串流使用 `[*]{fields}:` 作為標頭 — `*` 表示一個無界限 (unbounded) 的串流 (相對於固定行數批次處理的 `[N]`)。

#### Python

```python
import ttoon
from ttoon import StreamSchema, types

schema = StreamSchema({"name": types.string, "score": types.int})

with ttoon.stream_writer(open("out.ttoon", "w"), schema=schema) as writer:
    writer.write({"name": "Alice", "score": 95})
    writer.write({"name": "Bob", "score": 87})

print(writer.result.rows_emitted)  # 2
```

輸出：
```text
[*]{name,score}:
"Alice", 95
"Bob", 87
```

#### JavaScript / TypeScript

```ts
import { streamWriter, StreamSchema, types } from '@ttoon/shared';

const schema = new StreamSchema({ name: types.string, score: types.int });
const chunks: string[] = [];

const writer = streamWriter((chunk) => chunks.push(chunk), { schema });
writer.write({ name: 'Alice', score: 95 });
writer.write({ name: 'Bob', score: 87 });
const result = await writer.close();
console.log(result.rowsEmitted); // 2
```

### 讀取

#### Python

```python
for row in ttoon.stream_read(open("data.ttoon"), schema=schema):
    print(row)  # {"name": "Alice", "score": 95}
```

#### JavaScript / TypeScript

```ts
import { streamRead, StreamSchema, types } from '@ttoon/shared';

const schema = new StreamSchema({ name: types.string, score: types.int });

for await (const row of streamRead(source, { schema })) {
  console.log(row); // { name: "Alice", score: 95 }
}
```

JS 讀取器接受 `TextSource`：`string`, `Iterable<string | Uint8Array>`, `AsyncIterable<string | Uint8Array>`, 或是 `ReadableStreamLike<string | Uint8Array>`。

## T-JSON 串流

T-JSON 串流使用最頂層的 JSON 物件陣列格式。

### 寫入

#### Python

```python
with ttoon.stream_writer_tjson(sink, schema=schema) as writer:
    writer.write({"name": "Alice", "score": 95})
    writer.write({"name": "Bob", "score": 87})
```

輸出：
```text
[{"name": "Alice", "score": 95}
,{"name": "Bob", "score": 87}
]
```

#### JavaScript / TypeScript

```ts
import { streamWriterTjson, StreamSchema, types } from '@ttoon/shared';

const writer = streamWriterTjson(sink, { schema });
writer.write({ name: 'Alice', score: 95 });
await writer.close();
```

### 讀取

#### Python

```python
for row in ttoon.stream_read_tjson(source, schema=schema):
    print(row)
```

對 T-JSON 串流 reader 而言，`mode` 只影響 schema 外欄位的處理方式，不會放寬 JSON 值語法本身。

#### JavaScript / TypeScript

```ts
for await (const row of streamReadTjson(source, { schema })) {
  console.log(row);
}
```

## Arrow 串流

Arrow 串流讀取器會產生 (yield) `RecordBatch` 物件；寫入器則接受 `RecordBatch` 物件。

### 寫入

#### Python

```python
with ttoon.stream_writer_arrow(sink, schema=schema) as writer:
    writer.write_batch(record_batch)

# T-JSON 變體
with ttoon.stream_writer_arrow_tjson(sink, schema=schema) as writer:
    writer.write_batch(record_batch)
```

#### JavaScript / TypeScript

```ts
import { streamWriterArrow, StreamSchema, types } from '@ttoon/shared';

const writer = streamWriterArrow(sink, { schema });
writer.writeBatch(recordBatch);
await writer.close();
```

### 讀取

#### Python

```python
for batch in ttoon.stream_read_arrow(source, schema=schema, batch_size=1024):
    print(batch)  # pyarrow.RecordBatch
```

#### JavaScript / TypeScript

```ts
for await (const batch of streamReadArrow(source, { schema, batchSize: 1024 })) {
  console.log(batch); // RecordBatch
}
```

## 選項

### 寫入器選項

| 選項 | T-TOON 寫入器 | T-JSON 寫入器 | 說明 |
| :--- | :--- | :--- | :--- |
| `schema` | 必要 | 必要 | 欄位定義 |
| `delimiter` | 是 | 否 | `","`, `"\t"`, `"|"` |
| `binary_format` / `binaryFormat` | 是 | 是 | `"hex"` 或是 `"b64"` |
| `codecs` | 僅物件寫入器 | 僅物件寫入器 | 覆寫 codec |

### 讀取器選項

| 選項 | 所有讀取器 | Arrow 讀取器 | 說明 |
| :--- | :--- | :--- | :--- |
| `schema` | 必要 | 必要 | 欄位定義 |
| `mode` | 是 | 是 | `"compat"` 或是 `"strict"`；對 T-JSON 串流來說，主要控制 schema 外欄位的處理方式 |
| `codecs` | 僅物件讀取器 | 否 | 覆寫 codec |
| `batch_size` / `batchSize` | 否 | 是 | 每個 Arrow 批次的行數 (預設 1024) |

## JS 來源/接收器的靈活性

JS 串流接受多種 source 和 sink 的型別：

**TextSource:** `string | Iterable<string | Uint8Array> | AsyncIterable<string | Uint8Array> | ReadableStreamLike<string | Uint8Array>`

**TextSink:** `(chunk: string) => void | Promise<void>` | `{ write(chunk: string): void | Promise<void> }` | `WritableStreamLike<string>`

這代表您可以使用 callback、Node.js stream、Web Stream 或任何具有 `.write()` 方法的物件。

## StreamResult

所有寫入器在關閉時都會回傳一個 `StreamResult`：

| 語言 | 存取方式 | 屬性 |
| :--- | :--- | :--- |
| Python | `writer.result` 或是 `writer.close()` | `rows_emitted: int` |
| JS | `writer.result` 或是 `await writer.close()` | `rowsEmitted: number` |
| Rust | `writer.close()` | `rows_emitted: usize` |
