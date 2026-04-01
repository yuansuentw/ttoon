---
title: Python Guide
sidebar_position: 1
sidebar_label: Python
description: Complete guide to using TTOON with Python — batch, Arrow, Polars, streaming, and codecs.
---

# Python Guide

The `ttoon` Python package wraps the Rust core engine via PyO3. It provides Python-native types on the object path and zero-copy Arrow integration on the tabular path.

## Installation

```bash
pip install ttoon

# For Arrow / Polars support in minimal or source-based environments
pip install pyarrow polars
```

The wheel already depends on `pyarrow>=23.0.0` and `polars>=1.37.1`; the extra command is only needed for minimal or source-based environments.

## Batch Operations

### Serialize: `dumps()`

```python
import datetime as dt
import decimal
import uuid
import ttoon

text = ttoon.dumps({
    "name": "Alice",
    "amount": decimal.Decimal("123.45"),
    "id": uuid.UUID("550e8400-e29b-41d4-a716-446655440000"),
    "created_at": dt.datetime(2026, 3, 8, 14, 30, 0),
})
```

`dumps()` accepts Python native objects, `pyarrow.Table`, `pyarrow.RecordBatch`, and `polars.DataFrame`. When given Arrow or Polars input, it routes to the high-performance Arrow path automatically.

**Options:**

| Parameter | Type | Default | Description |
| :--- | :--- | :--- | :--- |
| `delimiter` | `str` | `","` | Tabular separator: `","`, `"\t"`, or `"\|"` |
| `indent_size` | `int \| None` | `None` | Indentation width; `None` uses the Rust default (`2`) |
| `binary_format` | `str` | `"hex"` | Binary encoding: `"hex"` or `"b64"` |

### Deserialize: `loads()`

```python
data = ttoon.loads(text)
data = ttoon.loads(text, mode="strict")
```

Auto-detects format (T-TOON / T-JSON / typed unit). The `mode` parameter only affects T-TOON parsing:

- `"compat"` (default) — unknown bare tokens fall back to strings
- `"strict"` — unknown bare tokens cause an error

### Generate T-JSON: `to_tjson()`

```python
text = ttoon.to_tjson({
    "created_at": dt.datetime(2026, 3, 8, 14, 30, 0),
    "score": 12.5,
})
# {"created_at": 2026-03-08T14:30:00, "score": 12.5}
```

`to_tjson()` does not accept Arrow/Polars input. For Arrow → T-JSON, use `stringify_arrow_tjson()`.

### Arrow Serialization: `stringify_arrow_tjson()`

```python
text = ttoon.stringify_arrow_tjson(table)
```

Serializes a PyArrow Table or Polars DataFrame to T-JSON format (list-of-objects).

### Format Detection: `detect_format()`

```python
fmt = ttoon.detect_format(text)  # "ttoon" | "tjson" | "typed_unit"
```

## Arrow / Polars Path

### Serialize

```python
import polars as pl
import ttoon

df = pl.DataFrame({"name": ["Alice", "Bob"], "score": [95, 87]})
text = ttoon.dumps(df)
# [2]{name,score}:
# "Alice", 95
# "Bob", 87
```

`dumps()` detects Polars DataFrames and PyArrow Tables, converting them to Arrow internally before Rust serializes directly from columnar data.

### Deserialize to Arrow

```python
table = ttoon.read_arrow(text)  # returns pyarrow.Table
```

Input must be a list of uniform objects. Field types are inferred from the data. Structural fields (list/object) are not arrowable.

## Direct Transcode

Convert between formats without materializing Python objects:

```python
# T-JSON → T-TOON
ttoon_text = ttoon.tjson_to_ttoon(
    '{"name": "Alice", "scores": [95, 87]}',
    delimiter=",",
)

# T-TOON → T-JSON
tjson_text = ttoon.ttoon_to_tjson(
    'name: "Alice"\nage: 30',
    mode="compat",
)
```

The text passes through Rust IR only — all typed semantics are fully preserved.

## Codec Registration

Register global codecs to customize value conversion:

```python
ttoon.use({
    "decimal": my_decimal_codec,
    "date": my_date_codec,
})
```

Codec values may be either:

- a mapping with `"encode"` / `"decode"` keys whose values are callables
- an object exposing `encode(value)` / `decode(value)` methods

Each hook is optional, but a codec must provide at least one callable hook. Python codecs affect object-path streaming readers and writers only. They do not change `loads()`, `to_tjson()`, Arrow readers/writers, or the direct transcode APIs.

## Streaming

For row-by-row processing, see the [Streaming Guide](streaming.md). Python streaming APIs support context managers and Python iterators:

```python
import ttoon
from ttoon import StreamSchema, types

schema = StreamSchema({"name": types.string, "score": types.int})

# Writing
with ttoon.stream_writer(sink, schema=schema) as writer:
    writer.write({"name": "Alice", "score": 95})
    writer.write({"name": "Bob", "score": 87})

# Reading
for row in ttoon.stream_read(source, schema=schema):
    print(row)
```

## Error Handling

```python
from ttoon import TranscodeError

try:
    ttoon.tjson_to_ttoon(invalid_text)
except TranscodeError as e:
    print(e.operation)       # "tjson_to_ttoon" | "ttoon_to_tjson"
    print(e.phase)           # "parse" | "serialize"
    print(e.source_kind)     # underlying source kind
    print(e.source_message)  # underlying source message
    print(e.source)          # {"kind", "message", "span"}
```

Parse errors include line/column information for diagnostics.

## Next Steps

- **[Arrow & Polars Guide](arrow-and-polars.md)** — Deep dive into tabular paths
- **[Streaming Guide](streaming.md)** — Row-by-row processing
- **[Python API Reference](../reference/python-api.md)** — Complete API signatures
