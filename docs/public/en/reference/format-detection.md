---
title: Format Detection
sidebar_position: 9
sidebar_label: Format Detection
description: How TTOON auto-detects input format between T-TOON, T-JSON, and typed unit.
---

# Format Detection

All TTOON parse APIs (`loads()`, `parse()`, `from_ttoon()`, `read_arrow()`, `readArrow()`) begin with automatic format detection. The `detect_format()` / `detectFormat()` function exposes this logic directly.

## Return Values

| Result | Meaning |
| :--- | :--- |
| `"tjson"` / `'tjson'` | T-JSON (bracket-based) |
| `"ttoon"` / `'ttoon'` | T-TOON (indentation-based or tabular) |
| `"typed_unit"` / `'typed_unit'` | A single typed value (e.g., `42`, `true`, `uuid(...)`) |

## Detection Rules

The detector examines the first meaningful (non-whitespace) character of the first line:

1. **First non-whitespace is `{`** → `tjson`
2. **First non-whitespace is `[`** and the line matches a TOON header pattern → `ttoon`
3. **First non-whitespace is `[`** and the line does not match a TOON header pattern → `tjson`
4. **Anything else** → `typed_unit`

The parser may still route `typed_unit` input into the T-TOON parser after detection, but `detect_format()` itself does not inspect `key: value` content.

## Usage

### Python

```python
import ttoon

ttoon.detect_format('{"name": "Alice"}')                    # "tjson"
ttoon.detect_format('[2]{name,score}:\n"Alice", 95\n"Bob", 87')  # "ttoon"
ttoon.detect_format('name: "Alice"\nage: 30')               # "typed_unit"
ttoon.detect_format('42')                                    # "typed_unit"
ttoon.detect_format('true')                                  # "typed_unit"
```

### JavaScript / TypeScript

```ts
import { detectFormat } from '@ttoon/shared';

detectFormat('{"key": 42}');       // 'tjson'
detectFormat('key: 42');           // 'typed_unit'
detectFormat('true');              // 'typed_unit'
```

### Rust

```rust
use ttoon_core::detect_format;
use ttoon_core::format_detect::Format;

let fmt = detect_format("{\"key\": 42}");
assert_eq!(fmt, Format::Tjson);

let fmt = detect_format("key: 42");
assert_eq!(fmt, Format::TypedUnit);
```

## Key Behaviors

### No Fallback

Once a format is determined, the parser commits to it. If parsing fails, the error is reported for the detected format — the parser does **not** silently retry with another format.

### T-JSON Detection

T-JSON detection triggers on `{`, and on `[` only when the line does not match a TOON header pattern. This means:

- `[1, 2, 3]` → T-JSON array
- `[2]{a,b}:` → T-TOON tabular (not T-JSON, despite starting with `[`)

The detector distinguishes between T-JSON arrays and T-TOON tabular headers by examining the pattern after `[`.

### Streaming Headers

The `[*]{fields}:` streaming header is detected as T-TOON, not T-JSON. The `*` marker distinguishes streaming from batch tabular (`[N]{fields}:`).

### Empty Input

Empty strings or whitespace-only input are handled by the detector. `detect_format()` returns `typed_unit` for that case; the parse APIs then treat empty input as an empty object `{}`.
