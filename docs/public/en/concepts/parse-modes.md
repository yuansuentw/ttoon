---
title: Parse Modes
sidebar_position: 4
sidebar_label: Parse Modes
description: Understanding compat and strict parse modes in TTOON.
---

# Parse Modes

TTOON supports two parse modes that control how unknown bare tokens are handled during T-TOON parsing.

## `compat` Mode

Unknown bare tokens fall back to strings. This is compatible with TOON v3.0 behavior where bare strings were valid.

```text
key: hello
```

In `compat` mode, `hello` is parsed as the string `"hello"`.

## `strict` Mode

Unknown bare tokens cause an immediate error. This is suitable for machine-generated data where every value should be explicitly typed.

```text
key: hello    → ERROR: unknown bare token "hello"
key: "hello"  → OK: string "hello"
key: 42       → OK: int 42
key: true     → OK: bool true
```

## Which Mode Affects What

| Format | Affected by `mode`? |
| :--- | :--- |
| T-TOON indentation | Yes |
| T-TOON tabular | Yes |
| T-JSON | **No** — always strict |
| Typed unit | Yes |

T-JSON is always strict regardless of the `mode` setting, because T-JSON follows JSON structural rules where all string values are quoted.

## Defaults by Language Surface

- Python `loads()` and `ttoon_to_tjson()` default to `compat`
- JS `parse()` and `ttoonToTjson()` default to `compat`
- Rust convenience APIs such as `from_ttoon()` default to `compat`
- Rust `ParseMode::default()` is `Strict`

## Usage

### Python

```python
import ttoon

# compat (default)
data = ttoon.loads('key: hello')         # {"key": "hello"}

# strict
data = ttoon.loads('key: "hello"', mode="strict")  # OK
data = ttoon.loads('key: hello', mode="strict")     # Error
```

### JavaScript / TypeScript

```ts
import { parse } from '@ttoon/shared';

parse('key: hello');                         // { key: "hello" }
parse('key: hello', { mode: 'strict' });     // Error
parse('key: "hello"', { mode: 'strict' });   // OK
```

### Rust

```rust
use ttoon_core::{from_ttoon, from_ttoon_with_mode, ParseMode};

let node = from_ttoon("key: hello")?;                             // compat convenience API
let node = from_ttoon_with_mode("key: hello", ParseMode::Strict); // Error
let mode = ParseMode::default();                                  // Strict
```

## Recommendations

| Scenario | Mode |
| :--- | :--- |
| Human-written config/data | `compat` |
| Machine-generated output | `strict` |
| Cross-language exchange | `strict` (ensures explicit types) |
| Legacy TOON v3.0 data | `compat` |
| Streaming with schema | Either (schema enforces types separately) |

## Interaction with Transcode

- `tjson_to_ttoon()` — T-JSON parse is always strict; no `mode` parameter
- Python/JS `ttoon_to_tjson()` / `ttoonToTjson()` — accepts `mode`, defaults to `compat`
- Rust `ttoon_to_tjson()` — `mode` is required; there is no Rust-side default parameter
