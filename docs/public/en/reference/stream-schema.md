---
title: Stream Schema
sidebar_position: 8
sidebar_label: Stream Schema
description: StreamSchema API reference for defining typed field schemas across all languages.
---

# Stream Schema

`StreamSchema` defines the field names and types for streaming operations. All streaming readers and writers require a schema.

## Construction

### Python

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

### JavaScript / TypeScript

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

### Rust

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

## Types Namespace

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

## Nullable Fields

All type specs support `.nullable()` to allow null values in the column:

```python
schema = StreamSchema({
    "name": types.string,           # NOT NULL
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

## Schema Access

### Python

```python
schema["name"]     # returns a field spec built from ttoon.types
len(schema)        # number of fields
list(schema)       # field names
schema.export()    # serializable form
```

### JavaScript

```ts
schema.get("name")    // FieldTypeSpec | undefined
schema.keys()         // IterableIterator<string>
schema.values()       // IterableIterator<FieldTypeSpec>
schema.entries()      // IterableIterator<[string, FieldTypeSpec]>
schema.export()       // serializable form
```

### Rust

```rust
schema.field("name")   // Option<&StreamField>
schema.fields()        // &[StreamField]
schema.len()           // usize
schema.is_empty()      // bool
```

## Validation Rules

All three language surfaces enforce the same conceptual rules:

- schemas must contain at least one field
- field names must be strings
- duplicate field names are rejected
- field types must come from the language-specific typed schema surface

Error surface by language:

- Python: invalid names/types raise `TypeError`; duplicates/empty schemas raise `ValueError`
- JavaScript: invalid names/types raise `TypeError`; duplicates/empty schemas raise `Error`
- Rust: `StreamSchema::try_new()` returns `Result`; `StreamSchema::new()` panics on invalid input

## Decimal Constraints

`decimal(precision, scale)` is forwarded to the Rust backend. Effective backend limits are:

- `precision` must be between `1` and `76`
- `scale` must fit Rust `i8`
- Arrow conversion uses `Decimal128` for `precision <= 38`, otherwise `Decimal256`

Out-of-range values may be accepted by the Python/JS wrapper constructors but will fail once the schema is validated or converted in Rust.

## Arrow Schema Conversion (Rust)

```rust
// StreamSchema → Arrow Schema
let arrow_schema = schema.to_arrow_schema()?;

// Arrow Schema → StreamSchema
let stream_schema = StreamSchema::from_arrow_schema(&arrow_schema)?;
```

## Relationship to Streaming

StreamSchema is required by all streaming operations:

- **Readers**: Schema defines which fields to expect and their types for parsing
- **Writers**: Schema defines the output header and value serialization rules
- **T-TOON streaming**: Schema maps to the `[*]{fields}:` header
- **T-JSON streaming**: Schema defines expected keys in each JSON object
