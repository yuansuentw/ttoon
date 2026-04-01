---
title: JavaScript / TypeScript Guide
sidebar_position: 2
sidebar_label: JS / TS
description: Complete guide to using TTOON with JavaScript and TypeScript — batch, Arrow, codecs, streaming, and transcode.
---

# JavaScript / TypeScript Guide

The `@ttoon/shared` package uses a WASM bridge to invoke the Rust core engine for parsing and serialization. The JS layer adds a codec system for custom type mapping (since JS lacks native types for `Decimal`, `UUID`, etc.).

## Installation

```bash
npm install @ttoon/shared

# For Arrow table operations (optional peer dependency)
npm install @ttoon/shared apache-arrow

# For custom decimal codecs (install the library your codec uses)
npm install @ttoon/shared decimal.js
npm install @ttoon/shared big.js
```

> `@ttoon/node` and `@ttoon/web` are re-exports of `@ttoon/shared` for environment-specific imports. Install `@ttoon/shared` directly unless you need explicit separation.

## Batch Operations

### Serialize: `stringify()`

```ts
import { stringify, toon } from '@ttoon/shared';

const text = stringify({
  name: 'Alice',
  age: 30,
  id: toon.uuid('550e8400-e29b-41d4-a716-446655440000'),
  amount: toon.decimal('123.45'),
  businessDate: toon.date('2026-03-08'),
});
```

JS lacks native `Decimal`, `UUID`, `Date` (date-only), and `Time` types. Use `toon.*()` markers to indicate typed values during serialization:

- `toon.uuid(str)` — UUID
- `toon.decimal(str)` — Decimal with `m` suffix
- `toon.date(str)` — Date (`YYYY-MM-DD`)
- `toon.time(str)` — Time (`HH:MM:SS`)
- `toon.datetime(str)` — DateTime (ISO 8601)

**Options:**

| Parameter | Type | Default | Description |
| :--- | :--- | :--- | :--- |
| `indentSize` | `number` | `2` | Indentation width |
| `delimiter` | `string` | `','` | Tabular separator: `','`, `'\t'`, or `'\|'` |
| `binaryFormat` | `string` | `'hex'` | Binary encoding: `'hex'` or `'b64'` |

### Deserialize: `parse()`

```ts
import { parse } from '@ttoon/shared';

const data = parse(text);
const strict = parse(text, { mode: 'strict' });
```

Auto-detects format. Default JS value mapping:

| Typed Type | JS Result |
| :--- | :--- |
| `int` | `number` (throws if outside safe range) |
| `float` | `number` |
| `decimal` | `string` (stripped `m` suffix) |
| `date`, `time`, `datetime` | `string` |
| `uuid` | `string` |
| `binary` | `Uint8Array` |

To change these defaults, register codecs with `use()`.

### Generate T-JSON: `toTjson()`

```ts
import { toTjson, toon } from '@ttoon/shared';

const text = toTjson({
  id: toon.uuid('550e8400-e29b-41d4-a716-446655440000'),
  amount: toon.decimal('123.45'),
});
```

### Format Detection: `detectFormat()`

```ts
import { detectFormat } from '@ttoon/shared';

detectFormat('{"key": 42}');       // 'tjson'
detectFormat('key: 42');           // 'typed_unit'
detectFormat('true');              // 'typed_unit'
```

## Direct Transcode

Convert between T-JSON and T-TOON without materializing JS objects:

```ts
import { tjsonToTtoon, ttoonToTjson } from '@ttoon/shared';

const ttoonText = tjsonToTtoon('{"name": "Alice", "age": 30}');
const tjsonText = ttoonToTjson('name: "Alice"\nage: 30');

// With options
const result = ttoonToTjson(text, { mode: 'strict', binaryFormat: 'b64' });
```

The JS direct-transcode wrappers currently cannot distinguish parse vs serialize failures inside the single WASM call. `TranscodeError.phase` is therefore always reported as `'parse'`; use `sourceKind` and the underlying message for diagnostics.

## Arrow Path

Requires the optional peer dependency `apache-arrow`.

```ts
import { readArrow, stringifyArrow, stringifyArrowTjson } from '@ttoon/shared';
import { tableFromArrays } from 'apache-arrow';

const table = tableFromArrays({
  name: ['Alice', 'Bob'],
  score: [95, 87],
});

// Arrow → T-TOON tabular
const ttoonText = await stringifyArrow(table);

// Arrow → T-JSON
const tjsonText = await stringifyArrowTjson(table);

// Text → Arrow
const restored = await readArrow(ttoonText);
```

Arrow APIs are `async` because they dynamically import the `apache-arrow` module.

## Codec System

JS returns strings by default for `decimal`, `date`, `time`, `datetime`, and `uuid` to avoid forcing third-party dependencies. Register codecs to change this:

```ts
import { use, intBigInt } from '@ttoon/shared';

// Switch int to BigInt for int64 safety
await use({ int: intBigInt() });
```

### Built-in Int Codecs

```ts
import { intNumber, intBigInt } from '@ttoon/shared';

// Default: throw on overflow (safe)
await use({ int: intNumber() });

// Accept precision loss explicitly
await use({ int: intNumber({ overflow: 'lossy' }) });

// Use BigInt for all integers
await use({ int: intBigInt() });
```

### Custom Codecs

```ts
import Decimal from 'decimal.js';
import { use, type Codec } from '@ttoon/shared';

const decimalCodec: Codec<Decimal> = {
  type: 'decimal',
  fromPayload(payload) {
    if (typeof payload !== 'string') throw new Error('expected decimal payload');
    return new Decimal(payload.slice(0, -1)); // strip 'm' suffix
  },
  toPayload(value) {
    return `${value.toString()}m`;
  },
  is(value): value is Decimal {
    return value instanceof Decimal;
  },
};

await use({ decimal: decimalCodec });
```

Codecs affect JS object-path value mapping in `parse()`, `stringify()`, `toTjson()`, and object-path streaming APIs. They do not alter T-TOON/T-JSON syntax, Rust/Python behavior, or Arrow schema inference.

For more on codecs and int64 strategies, see [JS Codecs & Int64](js-codecs-and-int64.md).

## Streaming

For row-by-row processing, see the [Streaming Guide](streaming.md). JS streaming readers return `AsyncIterable`, writers are push-based classes:

```ts
import { streamRead, streamWriter, StreamSchema, types } from '@ttoon/shared';

const schema = new StreamSchema({ name: types.string, score: types.int });

// Reading (async iterable)
for await (const row of streamRead(source, { schema })) {
  console.log(row);
}

// Writing (push-based)
const writer = streamWriter(sink, { schema });
writer.write({ name: 'Alice', score: 95 });
await writer.close();
```

## Error Handling

```ts
import { ToonError, TranscodeError } from '@ttoon/shared';

try {
  parse(invalidText, { mode: 'strict' });
} catch (e) {
  if (e instanceof ToonError) {
    console.log(e.kind);    // 'parse' | 'serialize' | 'detect' | 'transcode'
    console.log(e.message); // includes line/column details
  }
}
```

For `tjsonToTtoon()` and `ttoonToTjson()`, prefer `e.sourceKind` and `e.source.message` over `e.phase`; the current JS wrapper always reports `phase: 'parse'`.

## Next Steps

- **[JS Codecs & Int64](js-codecs-and-int64.md)** — Deep dive into codecs and BigInt strategies
- **[Arrow & Polars Guide](arrow-and-polars.md)** — Tabular path details
- **[Streaming Guide](streaming.md)** — Row-by-row processing
- **[JS API Reference](../reference/js-api.md)** — Complete API signatures
