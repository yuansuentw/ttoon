---
title: Quick Start
sidebar_position: 2
sidebar_label: Quick Start
description: Serialize, deserialize, and transcode with TTOON in under 5 minutes.
---

# Quick Start

This guide covers the shortest path to productive use:

1. Serialize and deserialize objects
2. Generate T-JSON output
3. Work with tabular data and Arrow
4. Transcode between formats

## 1. Object Round Trip

### Python

```python
import ttoon

data = {"name": "Alice", "age": 30, "id": "A-001"}

text = ttoon.dumps(data)
print(text)
# name: "Alice"
# age: 30
# id: "A-001"

restored = ttoon.loads(text)
```

### JavaScript / TypeScript

```ts
import { parse, stringify } from '@ttoon/shared';

const text = stringify({ name: 'Alice', age: 30, enabled: true });
const restored = parse(text);
```

### Rust

```rust
use ttoon_core::{from_ttoon, to_ttoon};

let node = from_ttoon("name: \"Alice\"\nage: 30")?;
let text = to_ttoon(&node, None)?;
```

## 2. Generating T-JSON

T-JSON uses JSON-like `{}` / `[]` brackets while keeping typed syntax at the value layer.

### Python

```python
import datetime as dt
import ttoon

text = ttoon.to_tjson({
    "created_at": dt.datetime(2026, 3, 8, 10, 30, 0),
    "score": 12.5,
})
print(text)
# {"created_at": 2026-03-08T10:30:00, "score": 12.5}
```

### JavaScript / TypeScript

```ts
import { toon, toTjson } from '@ttoon/shared';

const text = toTjson({
  id: toon.uuid('550e8400-e29b-41d4-a716-446655440000'),
  amount: toon.decimal('123.45'),
});
// {"amount": 123.45m, "id": uuid(550e8400-e29b-41d4-a716-446655440000)}
```

JS lacks native `Decimal` and `UUID` types, so `toon.*()` markers are used during serialization.

## 3. Tabular Data & Arrow

When data is a list of uniform objects, T-TOON automatically outputs tabular format:

```text
[2]{name,score}:
"Alice", 95
"Bob", 87
```

### Python: Polars / PyArrow

```python
import polars as pl
import ttoon

df = pl.DataFrame({"name": ["Alice", "Bob"], "score": [95, 87]})

text = ttoon.dumps(df)
table = ttoon.read_arrow(text)  # returns pyarrow.Table
```

- `dumps(df)` converts to Arrow internally, then serializes via the Rust core — no `list[dict]` intermediate
- `read_arrow()` returns a `pyarrow.Table` directly

### JavaScript: Apache Arrow

```ts
import { readArrow, stringifyArrow } from '@ttoon/shared';
import { tableFromArrays } from 'apache-arrow';

const table = tableFromArrays({
  name: ['Alice', 'Bob'],
  score: [95, 87],
});

const text = await stringifyArrow(table);
const restored = await readArrow(text);
```

## 4. Direct Transcode

Convert between T-JSON and T-TOON without materializing language-native objects — the text passes through Rust IR only.

### Python

```python
import ttoon

ttoon_text = ttoon.tjson_to_ttoon('{"name": "Alice", "scores": [95, 87]}')
tjson_text = ttoon.ttoon_to_tjson('name: "Alice"\nage: 30')
```

### JavaScript / TypeScript

```ts
import { tjsonToTtoon, ttoonToTjson } from '@ttoon/shared';

const ttoonText = tjsonToTtoon('{"name": "Alice", "age": 30}');
const tjsonText = ttoonToTjson('name: "Alice"\nage: 30');
```

### Rust

```rust
use ttoon_core::{tjson_to_ttoon, ttoon_to_tjson, ParseMode};

let ttoon = tjson_to_ttoon(r#"{"key": 42}"#, None)?;
let tjson = ttoon_to_tjson("key: 42", ParseMode::Compat, None)?;
```

## Next Steps

- **[Format Overview](format-overview.md)** — Understand the two syntaxes and typed value system
- **[Python Guide](../guides/python.md)** — Complete Python usage guide
- **[JS/TS Guide](../guides/js-ts.md)** — Complete JavaScript/TypeScript guide
- **[Arrow & Polars](../guides/arrow-and-polars.md)** — Deep dive into high-performance tabular paths
