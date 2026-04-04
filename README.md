# TTOON

TTOON is a typed plain text interchange system for modern data workflows.
It provides two complementary syntaxes under one project:

- `T-TOON`: indentation-based text extended from TOON v3.0
- `T-JSON`: JSON-compatible containers with the same typed value model at the leaf level

They are not separate products. Both syntaxes share the same typed model and the same Rust core implementation.

## Key Ideas

- Typed values are first-class: `date`, `time`, `datetime`, `decimal`, `uuid`, `hex`, and `b64` are part of the format model instead of string conventions.
- One core, multiple SDKs: Python, JavaScript, and Rust all route to the same Rust core for parsing and serialization semantics.
- Two processing paths: use object path for language-native values, or Arrow path for direct columnar workflows.

## Minimal Examples

`T-TOON`

```ttoon
user:
  id: uuid(550e8400-e29b-41d4-a716-446655440000)
  name: "Alice"
  joined_at: 2026-03-08T14:30:00Z
  balance: 123.45m
  avatar: b64(SGVsbG8=)
```

`T-JSON`

```json
{
  "user": {
    "id": uuid(550e8400-e29b-41d4-a716-446655440000),
    "name": "Alice",
    "joined_at": 2026-03-08T14:30:00Z,
    "balance": 123.45m,
    "avatar": b64(SGVsbG8=)
  }
}
```

Both forms carry the same typed model. Values such as `uuid(...)`, `2026-03-08T14:30:00Z`, `123.45m`, and `b64(...)` are typed values instead of plain strings.

## Language Index

- English: [en/README.md](docs/public/en/README.md)
- 繁體中文: [zh-TW/README.md](docs/public/zh-TW/README.md)
- Official site: [ttoon.dev](https://ttoon.dev/)