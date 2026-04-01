# TTOON


TTOON is a **typed plain text** data exchange format engineered for modern data workflows. It provides two complementary syntaxes:

- `T-TOON` — a clean, indentation-based structure with native tabular layout for datasets.
- `T-JSON` — a JSON-like structure that preserves typed value syntax at the leaf level.

TTOON's core goals:

- **Type fidelity** — Preserve data types across Python ↔ Rust ↔ JS boundaries.
- **Human readable** — Plain text you can read, diff, and debug visually.
- **High performance** — Arrow / Polars zero-copy paths for tabular data.
- **Cross-language** — Identical behavior via shared Rust core engine.

## Official SDKs

| Language | Package |
| :--- | :--- |
| **Python** | `ttoon` |
| **JavaScript / TypeScript** | `@ttoon/shared` |
| **JavaScript / Node.js** | `@ttoon/node` |
| **JavaScript / Web** | `@ttoon/web` |
| **Rust** | `ttoon-core` |

## Quick Examples

### Python

```python
import ttoon

text = ttoon.dumps({"name": "Alice", "amount": 123.45})
data = ttoon.loads(text)
```

### TypeScript

```ts
import { parse, stringify } from '@ttoon/shared';

const text = stringify({ name: 'Alice', enabled: true });
const data = parse(text);
```

## Documentation

- **English**: [en/](/docs/public/en/README.md)
- **繁體中文**: [zh-TW/](/docs/public/zh-TW/README.md)
