---
title: Streaming Guide
sidebar_position: 6
sidebar_label: Streaming
description: Row-by-row streaming readers and writers for T-TOON and T-JSON with object and Arrow variants.
---

# Streaming Guide

TTOON provides 8 streaming reader/writer combinations across two formats (T-TOON, T-JSON) and two paths (object, Arrow). All streaming operations require a `StreamSchema` that defines field names and types.

## Overview

| | T-TOON Object | T-TOON Arrow | T-JSON Object | T-JSON Arrow |
| :--- | :--- | :--- | :--- | :--- |
| **Reader** | `StreamReader` | `ArrowStreamReader` | `TjsonStreamReader` | `TjsonArrowStreamReader` |
| **Writer** | `StreamWriter` | `ArrowStreamWriter` | `TjsonStreamWriter` | `TjsonArrowStreamWriter` |

## Schema Definition

All streaming operations start with a `StreamSchema`:

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

### Available Types

| Type Spec | Python | JavaScript | Description |
| :--- | :--- | :--- | :--- |
| String | `types.string` | `types.string` | String |
| Int | `types.int` | `types.int` | Integer |
| Float | `types.float` | `types.float` | Float |
| Bool | `types.bool` | `types.bool` | Boolean |
| Date | `types.date` | `types.date` | Date |
| Time | `types.time` | `types.time` | Time |
| DateTime | `types.datetime` | `types.datetime` | DateTime (tz-aware) |
| DateTime Naive | `types.datetime_naive` | `types.datetimeNaive` | DateTime (no tz) |
| UUID | `types.uuid` | `types.uuid` | UUID |
| Binary | `types.binary` | `types.binary` | Binary |
| Decimal | `types.decimal(p, s)` | `types.decimal(p, s)` | Decimal(precision, scale) |

All types support `.nullable()` to allow null values.

## T-TOON Streaming

### Writing

T-TOON streaming uses `[*]{fields}:` as the header — the `*` indicates an unbounded stream (versus `[N]` for fixed-count batch).

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

Output:
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

### Reading

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

JS readers accept `TextSource`: `string`, `Iterable<string | Uint8Array>`, `AsyncIterable<string | Uint8Array>`, or `ReadableStreamLike<string | Uint8Array>`.

## T-JSON Streaming

T-JSON streaming uses a top-level JSON array of objects format.

### Writing

#### Python

```python
with ttoon.stream_writer_tjson(sink, schema=schema) as writer:
    writer.write({"name": "Alice", "score": 95})
    writer.write({"name": "Bob", "score": 87})
```

Output:
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

### Reading

#### Python

```python
for row in ttoon.stream_read_tjson(source, schema=schema):
    print(row)
```

For T-JSON streaming readers, `mode` only affects how schema-unknown fields are handled. It does not make JSON value syntax less strict.

#### JavaScript / TypeScript

```ts
for await (const row of streamReadTjson(source, { schema })) {
  console.log(row);
}
```

## Arrow Streaming

Arrow streaming readers yield `RecordBatch` objects; writers accept `RecordBatch` objects.

### Writing

#### Python

```python
with ttoon.stream_writer_arrow(sink, schema=schema) as writer:
    writer.write_batch(record_batch)

# T-JSON variant
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

### Reading

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

## Options

### Writer Options

| Option | T-TOON Writers | T-JSON Writers | Description |
| :--- | :--- | :--- | :--- |
| `schema` | Required | Required | Field definitions |
| `delimiter` | Yes | No | `","`, `"\t"`, `"\|"` |
| `binary_format` / `binaryFormat` | Yes | Yes | `"hex"` or `"b64"` |
| `codecs` | Object writers only | Object writers only | Codec overrides |

### Reader Options

| Option | All Readers | Arrow Readers | Description |
| :--- | :--- | :--- | :--- |
| `schema` | Required | Required | Field definitions |
| `mode` | Yes | Yes | `"compat"` or `"strict"`; for T-JSON streaming, this controls unknown-field handling against the schema |
| `codecs` | Object readers only | No | Codec overrides |
| `batch_size` / `batchSize` | No | Yes | Rows per Arrow batch (default 1024) |

## JS Source/Sink Flexibility

JS streaming accepts multiple source and sink types:

**TextSource:** `string | Iterable<string | Uint8Array> | AsyncIterable<string | Uint8Array> | ReadableStreamLike<string | Uint8Array>`

**TextSink:** `(chunk: string) => void | Promise<void>` | `{ write(chunk: string): void | Promise<void> }` | `WritableStreamLike<string>`

This means you can use callbacks, Node.js streams, Web Streams, or any object with a `.write()` method.

## StreamResult

All writers return a `StreamResult` on close:

| Language | Access | Property |
| :--- | :--- | :--- |
| Python | `writer.result` or `writer.close()` | `rows_emitted: int` |
| JS | `writer.result` or `await writer.close()` | `rowsEmitted: number` |
| Rust | `writer.close()` | `rows_emitted: usize` |
