---
title: T-TOON vs T-JSON
sidebar_position: 2
sidebar_label: T-TOON vs T-JSON
description: Detailed comparison between T-TOON (indentation-based) and T-JSON (bracket-based) syntaxes.
---

# T-TOON vs T-JSON

TTOON provides two syntaxes for different scenarios. Both share the same typed value layer — the difference is purely structural.

## Side-by-Side Comparison

### Simple Object

**T-TOON:**
```text
name: "Alice"
age: 30
active: true
```

**T-JSON:**
```text
{"active": true, "age": 30, "name": "Alice"}
```

### Nested Object

**T-TOON:**
```text
user:
  name: "Alice"
  address:
    city: "Taipei"
    zip: "100"
```

**T-JSON:**
```text
{"user": {"address": {"city": "Taipei", "zip": "100"}, "name": "Alice"}}
```

### Array

**T-TOON:**
```text
scores[3]: 95, 87, 92
```

**T-JSON:**
```text
[95, 87, 92]
```

### Tabular Data (List of Uniform Objects)

**T-TOON:**
```text
[3]{name,score,grade}:
"Alice", 95, "A"
"Bob", 87, "B"
"Carol", 92, "A"
```

**T-JSON:**
```text
[{"grade": "A", "name": "Alice", "score": 95}, {"grade": "B", "name": "Bob", "score": 87}, {"grade": "A", "name": "Carol", "score": 92}]
```

### Typed Values

Both syntaxes use identical typed value encoding:

```text
amount: 123.45m
id: uuid(550e8400-e29b-41d4-a716-446655440000)
created: 2026-03-08T14:30:00+08:00
blob: hex(4A42)
```

## Structural Differences

| Aspect | T-TOON | T-JSON |
| :--- | :--- | :--- |
| Structure | Indentation-based | Bracket-based (`{}` / `[]`) |
| Object keys | Bare identifiers followed by `: ` | Quoted strings (`"key"`) |
| Tabular format | Native `[N]{fields}:` header | Array of objects |
| Readability | Optimized for humans | Closer to JSON |
| Escape rules | 5 escapes only (`\\` `\"` `\n` `\r` `\t`) | Full JSON escape set |
| Streaming header | `[*]{fields}:` (unbounded) | Top-level array of objects |
| Nesting | Uses indentation depth | Uses bracket depth |

## When to Choose Which

### Use T-TOON When

- Human readability is the priority (config files, logs, debugging)
- Data is tabular — the `[N]{fields}:` format is far more compact than repeated JSON objects
- You need easy `diff` and `grep` on structured data
- Working with streaming tabular data (`[*]{fields}:` header)

### Use T-JSON When

- Downstream systems expect JSON-like structure
- You need full JSON escape support (`\uXXXX`, `\b`, `\f`)
- Bracket-based nesting is preferred over indentation
- Interoperating with existing JSON tooling (editors, validators, log processors)

### Either Works

- Cross-language exchange — all SDKs auto-detect the format on parse
- Arrow / Polars integration — both formats support `readArrow()`

## Parsing Behavior

The parser auto-detects the format based on the first meaningful content:

1. First non-whitespace is `{` → T-JSON
2. First line is `[N]:` or `[N]{fields}:` → T-TOON tabular
3. First non-whitespace is `[` but it does not match a T-TOON tabular header → T-JSON
4. Otherwise → `typed_unit`

`detect_format()` does not have a separate "T-TOON indentation" result. Indentation-based objects and scalar typed values both surface as `typed_unit`, then the T-TOON parser distinguishes them on the parse path. Once a format route is chosen, the parser commits to it. There is no silent fallback.

## Direct Conversion

You can convert between formats without materializing language-native objects:

```python
import ttoon

# T-JSON → T-TOON (through Rust IR only)
ttoon_text = ttoon.tjson_to_ttoon('{"name": "Alice", "age": 30}')

# T-TOON → T-JSON (through Rust IR only)
tjson_text = ttoon.ttoon_to_tjson('name: "Alice"\nage: 30')
```

```ts
import { tjsonToTtoon, ttoonToTjson } from '@ttoon/shared';

const ttoonText = tjsonToTtoon('{"name": "Alice", "age": 30}');
const tjsonText = ttoonToTjson('name: "Alice"\nage: 30');
```

See the [Transcode Guide](../guides/transcode.md) for details.

## Line-Separated Rows

Bare comma-separated lines without a tabular header are **not valid T-TOON**:

```text
1, 2, 3
4, 5, 6
```

Use `T-TOON` tabular with a header or `T-JSON` arrays instead.
