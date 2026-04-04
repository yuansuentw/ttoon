---
title: Format Overview
sidebar_position: 3
sidebar_label: Format Overview
description: Beginner introduction to T-TOON, T-JSON, and the basic typed value rules.
---

# Format Overview

TTOON has two text syntaxes:

- **T-TOON**: indentation-based, optimized for reading and editing by humans
- **T-JSON**: bracket-based, closer to JSON tooling and habits
- **Shared typed value layer**: both syntaxes use the same value encoding rules

T-TOON is an extension of [TOON](https://toonformat.dev/), the original format from the [toon-format project](https://github.com/toon-format/toon). It keeps TOON's indentation-based object layout and compact tabular form, then adds explicit typed values and the companion T-JSON syntax.

Most parse APIs auto-detect the input format, so in many workflows you can choose the syntax based on readability and interoperability rather than parser configuration.

## T-TOON at a Glance

T-TOON removes redundant brackets and uses indentation to express structure.

### Object

```text
name: "Alice"
age: 30
active: true
```

### Nested Object

```text
user:
  name: "Alice"
  address:
    city: "Taipei"
```

### Tabular Data

```text
[2]{name,score}:
"Alice", 95
"Bob", 87
```

The `[N]{fields}:` header declares row count and column names. This is the compact form for a list of uniform objects.

## T-JSON at a Glance

T-JSON keeps JSON-like `{}` / `[]` structure, but values still use TTOON typed syntax.

### Object

```text
{"name": "Alice", "amount": 123.45m, "id": uuid(550e8400-e29b-41d4-a716-446655440000)}
```

### Array

```text
[1, 2, 3]
```

### Nested

```text
{"user": {"name": "Alice", "scores": [95, 87]}}
```

## Typed Values at a Glance

Both syntaxes share the same 12 built-in typed value encodings:

| Type | Example |
| :--- | :--- |
| `null` | `null` |
| `bool` | `true` |
| `int` | `42` |
| `float` | `3.14` |
| `decimal` | `123.45m` |
| `string` | `"Alice"` |
| `date` | `2026-03-08` |
| `time` | `14:30:00` |
| `datetime` | `2026-03-08T14:30:00+08:00` |
| `uuid` | `uuid(550e8400-e29b-41d4-a716-446655440000)` |
| `hex` | `hex(48656C6C6F)` |
| `b64` | `b64(SGVsbG8=)` |

## Three Rules to Remember

- **Strings are always quoted**: use `"..."`, not bare words
- **Exact decimal uses `m`**: `123.45m` is decimal, `123.45` is float
- **UUID and binary use wrappers**: `uuid(...)`, `hex(...)`, `b64(...)`

## Which Syntax Should I Use?

| Scenario | Recommended |
| :--- | :--- |
| Human-readable config, logs, or diffs | T-TOON |
| Large tabular datasets | T-TOON tabular |
| JSON-like downstream integration | T-JSON |
| Bracket-based nesting preferred | T-JSON |
| Cross-language object exchange | Either |

## Next Steps

- **[Typed Value Reference](../reference/typed-value-reference.md)** — Full type semantics, detailed syntax rules, cross-language mapping, Arrow mapping, and RDBMS correspondence
- **[Format Detection](../reference/format-detection.md)** — Exact auto-detection rules for `tjson`, `ttoon`, and `typed_unit`
- **[T-TOON vs T-JSON](../concepts/ttoon-vs-tjson.md)** — More detailed comparison between the two syntaxes
