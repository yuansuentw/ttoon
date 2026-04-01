# @ttoon/node

`@ttoon/node` is the Node.js entry package for TTOON. It re-exports the public API from `@ttoon/shared`.

## Installation

```bash
npm install @ttoon/node
```

## Quick Example

```ts
import { parse, stringify } from '@ttoon/node';

const text = stringify({ name: 'Alice' });
const data = parse(text);
```

See `docs/public/en/` for guides and API reference material.
