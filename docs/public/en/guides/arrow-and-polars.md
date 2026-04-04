---
title: Arrow & Polars
sidebar_position: 5
sidebar_label: Arrow & Polars
description: High-performance tabular data paths with Apache Arrow and Polars integration.
---

# Arrow & Polars Guide

TTOON maintains two independent processing paths: the **object path** (general-purpose) and the **Arrow path** (high-performance tabular). This guide covers the Arrow path.

## Why a Separate Arrow Path?

The Arrow path keeps tabular data in Arrow-native columnar form instead of language-native objects. Today, the strongest fast path is T-JSON → Arrow direct read; T-TOON tabular still interoperates through the compatibility `Node` route. For tabular data, this means:

- **No language-native row materialization on the Arrow side** — data stays columnar instead of becoming `dict` / JS object rows
- **Lower conversion overhead where direct paths exist** — especially for T-JSON → Arrow reads
- **Native type preservation** — `Decimal128`, `Date32`, `Timestamp`, `FixedSizeBinary(16)` (UUID) stay in their Arrow-native forms

## Python: Polars & PyArrow

### Serialize

```python
import polars as pl
import pyarrow as pa
import ttoon

# Polars DataFrame
df = pl.DataFrame({"name": ["Alice", "Bob"], "score": [95, 87]})
text = ttoon.dumps(df)
# [2]{name,score}:
# "Alice", 95
# "Bob", 87

# PyArrow Table
table = pa.table({"name": ["Alice", "Bob"], "score": [95, 87]})
text = ttoon.dumps(table)

# Arrow → T-JSON
text = ttoon.stringify_arrow_tjson(df)
# [{"name": "Alice", "score": 95}, {"name": "Bob", "score": 87}]
```

`dumps()` auto-detects Polars DataFrame and PyArrow Table/RecordBatch inputs, routing them to the Arrow path. Polars DataFrames are converted to Arrow first (zero-copy in Polars).

### Deserialize to Arrow

```python
table = ttoon.read_arrow(text)  # returns pyarrow.Table
```

From the returned `pyarrow.Table`, you can convert to any downstream format:

```python
df = pl.from_arrow(table)      # Polars DataFrame
pandas_df = table.to_pandas()  # Pandas DataFrame
```

### Delimiter Options

```python
text = ttoon.dumps(df, delimiter="|")
# [2]{name,score}:
# "Alice"| 95
# "Bob"| 87

text = ttoon.dumps(df, delimiter="\t")
```

## JavaScript: Apache Arrow

Requires the optional peer dependency `apache-arrow`.

### Serialize

```ts
import { stringifyArrow, stringifyArrowTjson } from '@ttoon/shared';
import { tableFromArrays } from 'apache-arrow';

const table = tableFromArrays({
  name: ['Alice', 'Bob'],
  score: [95, 87],
});

// Arrow → T-TOON tabular
const ttoonText = await stringifyArrow(table);

// Arrow → T-JSON
const tjsonText = await stringifyArrowTjson(table);
```

### Deserialize to Arrow

```ts
import { readArrow } from '@ttoon/shared';

const table = await readArrow(text);
```

Arrow APIs in JS are `async` because they dynamically import the `apache-arrow` module.

## Rust

```rust
use ttoon_core::{read_arrow, arrow_to_ttoon, arrow_to_tjson};

let table = read_arrow(text)?;
let ttoon = arrow_to_ttoon(&table, None)?;
let tjson = arrow_to_tjson(&table, None)?;
```

## Arrow Input Requirements

`read_arrow()` across all languages enforces these constraints:

| Requirement | Description |
| :--- | :--- |
| Root must be a list | Arrow bridge only handles tabular data |
| Each element must be an object | Object keys become schema fields |
| Field types must be consistent | Cannot mix different scalar types in the same column |
| No structural fields | List/object values are not arrowable |

## Arrow Schema Mapping

| Typed Type | Arrow Type |
| :--- | :--- |
| `int` | `Int64` |
| `float` | `Float64` |
| `decimal` | `Decimal128` or `Decimal256` (by precision) |
| `string` | `Utf8` |
| `bool` | `Boolean` |
| `date` | `Date32` |
| `time` | `Time64(Microsecond)` |
| `datetime` | `Timestamp(Microsecond[, tz])` |
| `uuid` | `FixedSizeBinary(16)` + UUID metadata |
| `hex`/`b64` | `Binary` |
| `null` | Nullable column; all-null infers as `Null` |

Arrow types are preserved at their native resolution — `decimal` is not downgraded to string, `uuid` uses `FixedSizeBinary(16)` with metadata.

## Performance Notes

### T-JSON Direct Path

The Rust core includes a two-pass direct path for T-JSON → Arrow (`read_arrow_tjson_direct`) that skips the Token/Node intermediate layer. This significantly reduces memory usage for large datasets and benefits all SDKs through the shared core.

### Sparse Schema Support

T-JSON `read_arrow()` supports sparse rows — missing keys are treated as null. Schema field order is inferred from the first occurrence order within the batch.

T-TOON tabular uses the header field order and width as-is.

### Datetime Timezone Consistency

The JS Arrow bridge does not allow mixing timezone-aware and naive datetimes within the same column. Mixing them causes a schema inference error.

## Next Steps

- **[Streaming Guide](streaming.md)** — Row-by-row Arrow streaming with `ArrowStreamReader` / `ArrowStreamWriter`
- **[Type Mapping](../getting-started/format-overview.md)** — Complete cross-language type table
- **[Stream API](../reference/stream-api.md)** — Streaming APIs and schema definitions
