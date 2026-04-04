# @ttoon/web

`@ttoon/web` is the browser and ESM entry package for TTOON. It re-exports the public API from `@ttoon/shared`.

Call `await initWasm()` once before using the parsing and serialization APIs.

## Installation

```bash
npm install @ttoon/web
```

## Quick Example

```ts
import { initWasm, parse, stringify } from '@ttoon/web';

await initWasm();

const text = stringify({ name: 'Alice' });
const data = parse(text);
```

See `docs/public/en/` for guides and API reference material.
