---
title: Introduction
sidebar_position: 1
sidebar_label: Introduction
description: TTOON — a typed plain text data exchange format for Python, JavaScript, and Rust.
---

# TTOON

TTOON is a **typed plain text** data exchange format engineered for modern data workflows. It provides two complementary syntaxes under one project:

- **T-TOON** — a clean, indentation-based structure with native tabular layout for datasets.
- **T-JSON** — a JSON-like structure that preserves typed value syntax at the leaf level.

TTOON is an **independent project**. `T-TOON` is the indentation-based syntax extended from TOON v3.0, while `T-JSON` is the JSON-like structural syntax built on the same typed value system. They are complementary representations within one project, not two unrelated formats.

## Why TTOON?

Most serialization formats force a trade-off: human readability or machine precision. TTOON refuses that compromise:

- **Type fidelity** — Preserves `decimal`, `date`, `time`, `datetime`, `uuid`, and `binary` across language boundaries, instead of degrading everything to strings.
- **Human readable** — Plain text output that is easy to read, diff, and debug visually.
- **High performance** — First-class Apache Arrow and Polars integration with zero-copy paths for tabular data.
- **Cross-language** — Identical behavior across Python, JavaScript/TypeScript, and Rust via a shared Rust core engine.
- **Lightweight runtime** — No full Node.js required; works in Vercel functions, Cloudflare Workers, and Supabase Edge functions.

## Official SDKs

| Language | Package | Architecture |
| :--- | :--- | :--- |
| Python | `ttoon` | Rust core via PyO3 bridge |
| JavaScript / TypeScript | `@ttoon/shared` | Rust core via WASM bridge |
| JavaScript / Node.js | `@ttoon/node` | Re-exports `@ttoon/shared` |
| JavaScript / Web | `@ttoon/web` | Re-exports `@ttoon/shared` |
| Rust | `ttoon-core` | Core engine (canonical implementation) |

All three language SDKs share the same Rust core, ensuring consistent parsing and serialization behavior. The API surface is fully aligned at 18/18 across all languages — covering batch, streaming, and transcode operations.

## Quick Example

### Python

```python
import ttoon

text = ttoon.dumps({"name": "Alice", "amount": 123.45})
data = ttoon.loads(text)
```

### JavaScript / TypeScript

```ts
import { parse, stringify } from '@ttoon/shared';

const text = stringify({ name: 'Alice', amount: 123.45 });
const data = parse(text);
```

### Rust

```rust
use ttoon_core::{from_ttoon, to_ttoon};

let node = from_ttoon("name: \"Alice\"\nage: 30")?;
let text = to_ttoon(&node, None)?;
```

## Core Capabilities

| Capability | Description |
| :--- | :--- |
| **Batch parse / serialize** | `T-TOON` and `T-JSON` in both directions, for objects and Arrow tables |
| **Streaming I/O** | Row-by-row readers and writers for both formats, with object and Arrow variants |
| **Direct transcode** | `T-JSON → T-TOON` and `T-TOON → T-JSON` without materializing language-native objects |
| **Format detection** | Auto-detect `tjson`, `ttoon`, or `typed_unit` from input text |
| **Schema system** | `StreamSchema` with typed field definitions for streaming operations |
| **Codec extensibility** | Custom type mapping in JS and Python (e.g., `Decimal`, `BigInt`, `Temporal`) |

## Next Steps

- **[Installation](getting-started/installation.md)** — Set up TTOON in your project
- **[Quick Start](getting-started/quick-start.md)** — Your first round trip in 2 minutes
- **[Format Overview](getting-started/format-overview.md)** — Understand T-TOON and T-JSON syntax
- **[Why TTOON?](concepts/why-ttoon.md)** — Deeper motivation and use cases
- **[API Matrix](reference/api-matrix.md)** — Full cross-language API comparison
