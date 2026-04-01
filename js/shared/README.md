# @ttoon/shared

`@ttoon/shared` is the core JavaScript and TypeScript package for TTOON.

## Features

- `parse()` / `stringify()`
- `toTjson()`
- `readArrow()` / `stringifyArrow()`
- `detectFormat()`
- `use()` for codec registration

## Installation

```bash
npm install @ttoon/shared
```

Install `apache-arrow` to enable Arrow APIs:

```bash
npm install @ttoon/shared apache-arrow
```

## Quick Example

```ts
import { parse, stringify, toon, toTjson } from '@ttoon/shared';

const text = stringify({ name: 'Alice', score: 95 });
const data = parse(text);

const typedJson = toTjson({
  id: toon.uuid('550e8400-e29b-41d4-a716-446655440000'),
});
```

See `docs/public/en/` for guides and API reference material.
