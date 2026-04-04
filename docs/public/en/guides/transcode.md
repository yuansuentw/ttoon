---
title: Direct Transcode
sidebar_position: 4
sidebar_label: Transcode
description: Convert directly between T-JSON and T-TOON without materializing language-native objects.
---

# Direct Transcode

TTOON provides direct transcode APIs that convert between T-JSON and T-TOON format by passing through Rust IR only — no Python objects, JS values, or Arrow tables are materialized. This preserves all typed semantics (decimal, uuid, date, binary, etc.) with minimal overhead.

## How It Works

```text
T-JSON text ──parse──→ Rust IR ──serialize──→ T-TOON text
T-TOON text ──parse──→ Rust IR ──serialize──→ T-JSON text
```

The text is parsed into the internal representation (IR), then immediately serialized to the target format. Language-native objects are never created, making this the most efficient way to convert between formats.

## Python

```python
import ttoon

# T-JSON → T-TOON
ttoon_text = ttoon.tjson_to_ttoon('{"name": "Alice", "scores": [95, 87]}')
# name: "Alice"
# scores:
#   [2]: 95, 87

# T-TOON → T-JSON
tjson_text = ttoon.ttoon_to_tjson('name: "Alice"\nage: 30')
# {"name": "Alice", "age": 30}
```

**Options:**

| Function | Parameters |
| :--- | :--- |
| `tjson_to_ttoon(text)` | `delimiter`, `indent_size`, `binary_format` |
| `ttoon_to_tjson(text)` | `mode`, `binary_format` |

`tjson_to_ttoon()` uses a dedicated strict T-JSON parser — it does not accept a `mode` parameter.

## JavaScript / TypeScript

```ts
import { tjsonToTtoon, ttoonToTjson } from '@ttoon/shared';

// T-JSON → T-TOON
const ttoonText = tjsonToTtoon('{"name": "Alice", "age": 30}');

// T-TOON → T-JSON
const tjsonText = ttoonToTjson('name: "Alice"\nage: 30');

// With options
const result = tjsonToTtoon(text, { delimiter: '\t', binaryFormat: 'b64' });
const result2 = ttoonToTjson(text, { mode: 'strict' });
```

**Options:**

| Function | Options Interface |
| :--- | :--- |
| `tjsonToTtoon(text, opts?)` | `TjsonToTtoonOptions` — extends `SerializeOptions` |
| `ttoonToTjson(text, opts?)` | `TtoonToTjsonOptions` — extends `TjsonSerializeOptions` + `mode` |

## Rust

```rust
use ttoon_core::{tjson_to_ttoon, ttoon_to_tjson, BinaryFormat, Delimiter, ParseMode, TjsonOptions, TtoonOptions};

// T-JSON → T-TOON (always strict parse)
let ttoon = tjson_to_ttoon(r#"{"key": 42}"#, None)?;

// T-TOON → T-JSON (configurable parse mode)
let tjson = ttoon_to_tjson("key: 42", ParseMode::Compat, None)?;

// With encode options
let opts = TtoonOptions {
    binary_format: BinaryFormat::Hex,
    indent_size: 4,
    delimiter: Delimiter::Comma,
};
let json_opts = TjsonOptions {
    binary_format: BinaryFormat::B64,
};
let ttoon_with_opts = tjson_to_ttoon(r#"{"key": 42}"#, Some(&opts))?;
let tjson_with_opts = ttoon_to_tjson("key: 42", ParseMode::Compat, Some(&json_opts))?;
```

## Error Handling

Transcode errors always include the operation. Phase reporting differs by language:

- Python and Rust: `phase` accurately reflects parse vs serialize
- JavaScript: `phase` is currently always reported as `'parse'` for direct transcode because the WASM bridge exposes the operation as a single call

```python
from ttoon import TranscodeError

try:
    ttoon.tjson_to_ttoon("invalid{json")
except TranscodeError as e:
    print(e)  # operation: tjson_to_ttoon, phase: parse, ...
```

```ts
import { TranscodeError } from '@ttoon/shared';

try {
  tjsonToTtoon('invalid{json');
} catch (e) {
  if (e instanceof TranscodeError) {
    console.log(e.operation); // 'tjson_to_ttoon'
    console.log(e.phase);     // currently always 'parse' in JS
    console.log(e.sourceKind); // underlying source error kind
  }
}
```

## Key Behaviors

- **All typed values preserved**: `decimal(m)`, `uuid(...)`, `date`, `time`, `datetime`, `hex(...)`, `b64(...)` all survive the conversion intact.
- **T-JSON parse is always strict**: `tjson_to_ttoon()` does not accept a `mode` parameter — T-JSON is strict by definition.
- **T-TOON parse uses `mode`**: `ttoon_to_tjson()` defaults to `compat` mode, but `strict` can be specified.
- **No object materialization**: No Python `dict`, JS `object`, or Arrow table is created during conversion.
- **JS phase caveat**: JS `TranscodeError.phase` is currently a coarse wrapper field, not a reliable parse/serialize discriminator.
