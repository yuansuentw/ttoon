# @ttoon/web

`@ttoon/web` is the browser and ESM entry package for TTOON. It re-exports the public API from `@ttoon/shared`.

## Installation

```bash
npm install @ttoon/web
```

## Quick Example

```ts
import { parse, stringify } from '@ttoon/web';

const text = stringify({ name: 'Alice' });
const data = parse(text);
```

See `docs/public/en/` for guides and API reference material.
