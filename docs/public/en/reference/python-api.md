---
title: Python API Reference
sidebar_position: 2
sidebar_label: Python API
description: Complete Python API reference for the ttoon package.
---

# Python API Reference

Package: `ttoon` (PyPI)

The current published Python package depends on `pyarrow>=23.0.0` and `polars>=1.37.1`.

## Batch APIs

### `dumps(obj, delimiter=",", indent_size=None, binary_format=None) → str`

Serialize a Python object to T-TOON text.

- Accepts: Python native objects, `pyarrow.Table`, `pyarrow.RecordBatch`, `polars.DataFrame`
- Arrow/Polars input routes to the Arrow path automatically
- Uniform object lists output as tabular format `[N]{fields}:`

### `loads(text, mode=None) → object`

Deserialize T-TOON / T-JSON / typed unit text to Python native objects.

- Auto-detects format
- `mode`: `"compat"` (default) or `"strict"` — only affects T-TOON parsing

### `to_tjson(obj, binary_format=None) → str`

Serialize a Python object to T-JSON text.

- Does **not** accept Arrow/Polars input — use `stringify_arrow_tjson()` for that

### `stringify_arrow_tjson(obj, binary_format=None) → str`

Serialize a PyArrow Table / RecordBatch or Polars DataFrame to T-JSON (list-of-objects).

### `read_arrow(text) → pyarrow.Table`

Parse T-TOON / T-JSON text to a PyArrow Table.

- Auto-detects format
- Input must be a list of uniform objects with scalar fields

### `detect_format(text) → str`

Detect the input format. Returns `"ttoon"`, `"tjson"`, or `"typed_unit"`.

## Transcode APIs

### `tjson_to_ttoon(text, *, delimiter=",", indent_size=None, binary_format=None) → str`

Convert T-JSON text directly to T-TOON text through Rust IR only.

- Always uses strict T-JSON parse — no `mode` parameter

### `ttoon_to_tjson(text, *, mode="compat", binary_format=None) → str`

Convert T-TOON text directly to T-JSON text through Rust IR only.

- `mode`: `"compat"` (default) or `"strict"`

## Codec API

### `use(codecs) → None`

Register global codecs for custom type conversion on the Python object-path streaming APIs.

Each codec value may be either:

- a mapping containing `"encode"` and/or `"decode"` keys whose values are callables
- an object exposing `encode(value)` and/or `decode(value)` methods

At least one callable hook must be provided. Codecs affect:

- `stream_read()` / `stream_writer()`
- `stream_read_tjson()` / `stream_writer_tjson()`

They do not affect `loads()`, `to_tjson()`, Arrow-path streaming, or direct transcode.

```python
ttoon.use({"decimal": my_decimal_codec})
```

## Streaming APIs

### Factory Functions

| Function | Returns | Format | Path |
| :--- | :--- | :--- | :--- |
| `stream_read(source, *, schema, mode=None, codecs=None)` | `StreamReader` | T-TOON | Object |
| `stream_read_tjson(source, *, schema, mode=None, codecs=None)` | `TjsonStreamReader` | T-JSON | Object |
| `stream_read_arrow(source, *, schema, batch_size=1024, mode=None)` | `ArrowStreamReader` | T-TOON | Arrow |
| `stream_read_arrow_tjson(source, *, schema, batch_size=1024, mode=None)` | `TjsonArrowStreamReader` | T-JSON | Arrow |
| `stream_writer(sink, *, schema, delimiter=",", binary_format=None, codecs=None)` | `StreamWriter` | T-TOON | Object |
| `stream_writer_tjson(sink, *, schema, binary_format=None, codecs=None)` | `TjsonStreamWriter` | T-JSON | Object |
| `stream_writer_arrow(sink, *, schema, delimiter=",", binary_format=None)` | `ArrowStreamWriter` | T-TOON | Arrow |
| `stream_writer_arrow_tjson(sink, *, schema, binary_format=None)` | `TjsonArrowStreamWriter` | T-JSON | Arrow |

### Stream Reader Classes

All readers are Python iterators:

```python
for row in reader:
    print(row)  # dict[str, Any] for object readers, RecordBatch for arrow readers
```

### Stream Writer Classes

All writers support context managers:

```python
with stream_writer(sink, schema=schema) as writer:
    writer.write({"name": "Alice", "score": 95})
result = writer.result  # StreamResult
```

| Class | Write Method | Notes |
| :--- | :--- | :--- |
| `StreamWriter` | `write(row: Mapping)` | Object rows |
| `TjsonStreamWriter` | `write(row: Mapping)` | Object rows |
| `ArrowStreamWriter` | `write_batch(batch)` | Arrow RecordBatch |
| `TjsonArrowStreamWriter` | `write_batch(batch)` | Arrow RecordBatch |

### `StreamResult`

Returned by `writer.close()` or accessible via `writer.result` after closing.

| Attribute | Type | Description |
| :--- | :--- | :--- |
| `rows_emitted` | `int` | Number of rows written |

## Schema API

### `StreamSchema(fields)`

Create a schema from a mapping or iterable of `(name, type_spec)` pairs.

```python
from ttoon import StreamSchema, types

schema = StreamSchema({
    "name": types.string,
    "score": types.int,
    "amount": types.decimal(10, 2),
    "active": types.bool.nullable(),
})
```

Supports `Mapping`-like access: `schema["name"]`, `len(schema)`, iteration over field names.

Validation rules:

- field names must be `str`
- field specs must be built from `ttoon.types`
- duplicate field names raise `ValueError`
- empty schemas raise `ValueError`

### `types` Namespace

| Type Spec | Description |
| :--- | :--- |
| `types.string` | String |
| `types.int` | Integer |
| `types.float` | Float |
| `types.bool` | Boolean |
| `types.date` | Date |
| `types.time` | Time |
| `types.datetime` | DateTime (timezone-aware) |
| `types.datetime_naive` | DateTime (naive, no timezone) |
| `types.uuid` | UUID |
| `types.binary` | Binary |
| `types.decimal(precision, scale)` | Decimal with specific precision and scale |

All type specs support `.nullable()` to mark the field as nullable.

## Error Types

### `TranscodeError`

Raised by `tjson_to_ttoon()` and `ttoon_to_tjson()` on conversion errors.

Available attributes:

| Attribute | Type | Description |
| :--- | :--- | :--- |
| `operation` | `str` | `"tjson_to_ttoon"` or `"ttoon_to_tjson"` |
| `phase` | `str` | `"parse"` or `"serialize"` |
| `source_kind` | `str` | Underlying source error kind |
| `source_message` | `str` | Underlying source error message |
| `source` | `dict` | `{"kind", "message", "span"}` where `span` is either `None` or `{"offset", "line", "column"}` |

## Serialization Options

| Parameter | APIs | Values | Default |
| :--- | :--- | :--- | :--- |
| `delimiter` | `dumps`, `tjson_to_ttoon`, stream writers | `","`, `"\t"`, `"\|"` | `","` |
| `indent_size` | `dumps`, `tjson_to_ttoon` | `int \| None` (effective Rust range: `0..=255`) | `None` |
| `binary_format` | all serialize/transcode APIs | `"hex"`, `"b64"` | `"hex"` |
| `mode` | `loads`, `ttoon_to_tjson`, stream readers | `"compat"`, `"strict"` | `"compat"` |
