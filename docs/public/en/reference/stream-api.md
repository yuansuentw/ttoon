---
title: Stream API
sidebar_position: 5
sidebar_label: Stream API
description: Streaming readers and writers for T-TOON and T-JSON across Python, JavaScript, and Rust.
---

# Stream API

This page groups the row-by-row streaming APIs for `T-TOON` and `T-JSON`. Use the language tabs to switch between Python, JavaScript, and Rust.

All streaming APIs require a `StreamSchema`.

Shared format conventions:

- **T-TOON stream**: `[*]{fields}:`
- **T-JSON stream**: top-level array of objects
- **Object path**: row values as language-native objects
- **Arrow path**: row batches as Arrow-native batches

import Tabs from '@theme/Tabs';
import TabItem from '@theme/TabItem';

<Tabs groupId="language">
<TabItem value="python" label="Python">

Package: `ttoon`

## Reader Factories

| Function | Returns | Format | Path |
| :--- | :--- | :--- | :--- |
| `stream_read(source, *, schema, mode=None, codecs=None)` | `StreamReader` | T-TOON | Object |
| `stream_read_tjson(source, *, schema, mode=None, codecs=None)` | `TjsonStreamReader` | T-JSON | Object |
| `stream_read_arrow(source, *, schema, batch_size=1024, mode=None)` | `ArrowStreamReader` | T-TOON | Arrow |
| `stream_read_arrow_tjson(source, *, schema, batch_size=1024, mode=None)` | `TjsonArrowStreamReader` | T-JSON | Arrow |

All readers are Python iterators:

```python
for row in reader:
    print(row)
```

For T-JSON streaming readers, `mode` does not relax JSON value syntax. It only controls how schema-unknown fields are handled: `compat` discards them, while `strict` rejects them.

## Writer Factories

| Function | Returns | Format | Path |
| :--- | :--- | :--- | :--- |
| `stream_writer(sink, *, schema, delimiter=",", binary_format=None, codecs=None)` | `StreamWriter` | T-TOON | Object |
| `stream_writer_tjson(sink, *, schema, binary_format=None, codecs=None)` | `TjsonStreamWriter` | T-JSON | Object |
| `stream_writer_arrow(sink, *, schema, delimiter=",", binary_format=None)` | `ArrowStreamWriter` | T-TOON | Arrow |
| `stream_writer_arrow_tjson(sink, *, schema, binary_format=None)` | `TjsonArrowStreamWriter` | T-JSON | Arrow |

All writers support context managers:

```python
with stream_writer(sink, schema=schema) as writer:
    writer.write({"name": "Alice", "score": 95})
result = writer.result
```

## Writer Methods and Result

| Class | Write Method | Notes |
| :--- | :--- | :--- |
| `StreamWriter` | `write(row: Mapping)` | Object rows |
| `TjsonStreamWriter` | `write(row: Mapping)` | Object rows |
| `ArrowStreamWriter` | `write_batch(batch)` | Arrow `RecordBatch` |
| `TjsonArrowStreamWriter` | `write_batch(batch)` | Arrow `RecordBatch` |

`StreamResult`:

| Attribute | Type | Description |
| :--- | :--- | :--- |
| `rows_emitted` | `int` | Number of rows written |

## Codec Scope

### `use(codecs) -> None`

Registers global codecs for Python object-path streaming APIs.

Codecs affect:

- `stream_read()` / `stream_writer()`
- `stream_read_tjson()` / `stream_writer_tjson()`

They do not affect batch `loads()`, batch `to_tjson()`, Arrow-path streaming, or direct transcode.

</TabItem>
<TabItem value="js" label="JavaScript / TypeScript">

Package: `@ttoon/shared`

## Reader Factories

All readers return `AsyncIterable`:

| Function | Returns | Format | Path |
| :--- | :--- | :--- | :--- |
| `streamRead(source, opts)` | `AsyncIterable<Record<string, unknown>>` | T-TOON | Object |
| `streamReadTjson(source, opts)` | `AsyncIterable<Record<string, unknown>>` | T-JSON | Object |
| `streamReadArrow(source, opts)` | `AsyncIterable<RecordBatch>` | T-TOON | Arrow |
| `streamReadArrowTjson(source, opts)` | `AsyncIterable<RecordBatch>` | T-JSON | Arrow |

`StreamReadOptions`:

| Property | Type | Required | Description |
| :--- | :--- | :--- | :--- |
| `schema` | `StreamSchema \| StreamSchemaInput` | Yes | Field definitions |
| `mode` | `ParseMode` | No | Parse mode; for T-JSON streaming this controls unknown-field policy against the schema |
| `codecs` | `CodecRegistry` | No | Codec overrides for object readers |

`StreamReadArrowOptions`:

| Property | Type | Required | Description |
| :--- | :--- | :--- | :--- |
| `schema` | `StreamSchema \| StreamSchemaInput` | Yes | Field definitions |
| `batchSize` | `number` | No | Rows per Arrow batch |
| `mode` | `ParseMode` | No | Parse mode; for T-JSON streaming this controls unknown-field policy against the schema |

## Writer Factories

| Function | Returns | Format | Path |
| :--- | :--- | :--- | :--- |
| `streamWriter(sink, opts)` | `StreamWriter` | T-TOON | Object |
| `streamWriterTjson(sink, opts)` | `TjsonStreamWriter` | T-JSON | Object |
| `streamWriterArrow(sink, opts)` | `ArrowStreamWriter` | T-TOON | Arrow |
| `streamWriterArrowTjson(sink, opts)` | `TjsonArrowStreamWriter` | T-JSON | Arrow |

Object writer options:

| Property | Type | Required | Description |
| :--- | :--- | :--- | :--- |
| `schema` | `StreamSchema \| StreamSchemaInput` | Yes | Field definitions |
| `delimiter` | `',' \| '\t' \| '\|'` | No | T-TOON tabular separator |
| `binaryFormat` | `'hex' \| 'b64'` | No | Binary encoding |
| `codecs` | `CodecRegistry` | No | Codec overrides for object writers |

Arrow writer options remove `codecs`; T-JSON writers remove `delimiter`.

## Writer Classes and Result

| Class | Method | Input |
| :--- | :--- | :--- |
| `StreamWriter` | `write(row)` | `Record<string, unknown>` |
| `TjsonStreamWriter` | `write(row)` | `Record<string, unknown>` |
| `ArrowStreamWriter` | `writeBatch(batch)` | `RecordBatch` |
| `TjsonArrowStreamWriter` | `writeBatch(batch)` | `RecordBatch` |

All writers expose:

- `close(): Promise<StreamResult>`
- `result: StreamResult | undefined`

`StreamResult`:

| Property | Type | Description |
| :--- | :--- | :--- |
| `rowsEmitted` | `number` | Number of rows written |

## Source / Sink Types

- `TextSource`: `string | Iterable<string | Uint8Array> | AsyncIterable<string | Uint8Array> | ReadableStreamLike<string | Uint8Array>`
- `TextSink`: `((chunk: string) => void | Promise<void>) | { write(chunk: string): void | Promise<void> } | WritableStreamLike<string>`

## Codec Scope

### `use(codecs): Promise<void>`

Registers global codecs for JS object-path parsing / serialization, including object-path stream readers and writers. Arrow-path streaming is schema-driven and does not use codecs.

</TabItem>
<TabItem value="rust" label="Rust">

Crate: `ttoon-core`

## Reader Types

| Type | Format | Output |
| :--- | :--- | :--- |
| `StreamReader` | T-TOON | `IndexMap<String, Node>` |
| `TjsonStreamReader` | T-JSON | `IndexMap<String, Node>` |
| `ArrowStreamReader` | T-TOON | Arrow `RecordBatch` |
| `TjsonArrowStreamReader` | T-JSON | Arrow `RecordBatch` |

Example:

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

## Writer Types

| Type | Format | Input |
| :--- | :--- | :--- |
| `StreamWriter` | T-TOON | Row values |
| `TjsonStreamWriter` | T-JSON | Row values |
| `ArrowStreamWriter` | T-TOON | Arrow `RecordBatch` |
| `TjsonArrowStreamWriter` | T-JSON | Arrow `RecordBatch` |

Example:

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

## Schema and Configuration

- `StreamSchema`, `StreamField`, `FieldType`, and `ScalarType` are documented below
- T-TOON stream writers use `TtoonOptions`
- T-JSON stream writers use `TjsonOptions`
- Readers that accept parse mode use `ParseMode`

</TabItem>
</Tabs>

## Stream Schema

`StreamSchema` defines the field names and types for streaming operations. All streaming readers and writers require a schema.

### Construction

<Tabs groupId="language">
<TabItem value="python" label="Python">

```python
from ttoon import StreamSchema, types

# From dict
schema = StreamSchema({
    "name": types.string,
    "score": types.int,
    "amount": types.decimal(10, 2),
})

# From list of tuples (preserves insertion order)
schema = StreamSchema([
    ("name", types.string),
    ("score", types.int),
])
```

</TabItem>
<TabItem value="js" label="JavaScript / TypeScript">

```ts
import { StreamSchema, types } from '@ttoon/shared';

// From object
const schema = new StreamSchema({
  name: types.string,
  score: types.int,
  amount: types.decimal(10, 2),
});

// Also accepts iterable input, including array-of-tuples
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

// Fallible construction
let schema = StreamSchema::try_new(fields)?;
```

</TabItem>
</Tabs>

### Types Namespace

| Type | Python | JavaScript | Rust |
| :--- | :--- | :--- | :--- |
| String | `types.string` | `types.string` | `ScalarType::String` |
| Int | `types.int` | `types.int` | `ScalarType::Int` |
| Float | `types.float` | `types.float` | `ScalarType::Float` |
| Bool | `types.bool` | `types.bool` | `ScalarType::Bool` |
| Date | `types.date` | `types.date` | `ScalarType::Date` |
| Time | `types.time` | `types.time` | `ScalarType::Time` |
| DateTime (tz) | `types.datetime` | `types.datetime` | `ScalarType::DateTime { has_tz: true }` |
| DateTime (naive) | `types.datetime_naive` | `types.datetimeNaive` | `ScalarType::DateTime { has_tz: false }` |
| UUID | `types.uuid` | `types.uuid` | `ScalarType::Uuid` |
| Binary | `types.binary` | `types.binary` | `ScalarType::Binary` |
| Decimal(p, s) | `types.decimal(p, s)` | `types.decimal(p, s)` | `ScalarType::decimal(p, s)` or `ScalarType::Decimal { precision, scale }` |

Rust also exposes convenience constructors `ScalarType::datetime()` and `ScalarType::datetime_naive()`.

### Nullable Fields

All type specs support `.nullable()` to allow null values in the column:

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

### Schema Access

<Tabs groupId="language">
<TabItem value="python" label="Python">

```python
schema["name"]     # returns a field spec built from ttoon.types
len(schema)        # number of fields
list(schema)       # field names
schema.export()    # serializable form
```

</TabItem>
<TabItem value="js" label="JavaScript / TypeScript">

```ts
schema.get("name")    // FieldTypeSpec | undefined
schema.keys()         // IterableIterator<string>
schema.values()       // IterableIterator<FieldTypeSpec>
schema.entries()      // IterableIterator<[string, FieldTypeSpec]>
schema.export()       // serializable form
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

### Validation Rules

All three language surfaces enforce the same conceptual rules:

- schemas must contain at least one field
- field names must be strings
- duplicate field names are rejected
- field types must come from the language-specific typed schema surface

Error surface by language:

- Python: invalid names/types raise `TypeError`; duplicates/empty schemas raise `ValueError`
- JavaScript: invalid names/types raise `TypeError`; duplicates/empty schemas raise `Error`
- Rust: `StreamSchema::try_new()` returns `Result`; `StreamSchema::new()` panics on invalid input

### Decimal Constraints

`decimal(precision, scale)` is forwarded to the Rust backend. Effective backend limits are:

- `precision` must be between `1` and `76`
- `scale` must fit Rust `i8`
- Arrow conversion uses `Decimal128` for `precision <= 38`, otherwise `Decimal256`

Out-of-range values may be accepted by the Python/JS wrapper constructors but will fail once the schema is validated or converted in Rust.

### Arrow Schema Conversion (Rust)

```rust
// StreamSchema -> Arrow Schema
let arrow_schema = schema.to_arrow_schema()?;

// Arrow Schema -> StreamSchema
let stream_schema = StreamSchema::from_arrow_schema(&arrow_schema)?;
```

## Related Pages

- **[T-TOON Batch API](./ttoon-batch-api.md)** — Non-streaming T-TOON APIs
- **[T-JSON Batch API](./tjson-batch-api.md)** — Non-streaming T-JSON APIs
