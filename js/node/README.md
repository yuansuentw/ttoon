# @ttoon/node

`@ttoon/node` is the Node.js entry package for TTOON. It re-exports the public API from `@ttoon/shared`.

Call `await initWasm()` once before using the parsing and serialization APIs.

## Installation

```bash
npm install @ttoon/node
```

## Quick Example

```ts
import { initWasm, parse, stringify } from '@ttoon/node';

await initWasm();

const text = stringify({ name: 'Alice' });
const data = parse(text);
```

See `docs/public/en/` for guides and API reference material.
