# ttoon

`ttoon` is the Python SDK for TTOON.

## Features

- `T-TOON` and `T-JSON` serialization and parsing
- Conversion between Python values and typed values
- Arrow-based workflows for `pyarrow` and `polars`

## Installation

```bash
pip install ttoon
```

## Quick Example

```python
import ttoon

text = ttoon.dumps({"name": "Alice", "score": 95})
data = ttoon.loads(text)

typed_json = ttoon.to_tjson({"name": "Alice"})
fmt = ttoon.detect_format(text)
```

## Arrow Example

```python
import polars as pl
import ttoon

df = pl.DataFrame({"name": ["Alice", "Bob"], "score": [95, 87]})
text = ttoon.dumps(df)
table = ttoon.read_arrow(text)
```

## Notes

- `dumps()` accepts standard Python objects as well as `polars.DataFrame`, `pyarrow.Table`, and `pyarrow.RecordBatch`.
- `read_arrow()` returns a `pyarrow.Table`.
- `loads()` automatically detects `T-TOON`, `T-JSON`, and `typed_unit` inputs.

See `docs/public/en/` for guides and API reference material.
