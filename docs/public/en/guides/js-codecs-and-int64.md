---
title: JS Codecs & Int64
sidebar_position: 7
sidebar_label: JS Codecs & Int64
description: Custom type mapping in JavaScript — codec system, int64 strategies, and BigInt.
---

# JS Codecs & Int64

JavaScript lacks native types for `Decimal`, `UUID`, date-only, and time-only values. The TTOON codec system lets you customize how these types are represented in JS.

## Default Behavior (No Codecs)

| Typed Type | JS Result | Why |
| :--- | :--- | :--- |
| `int` | `number` (throws if overflow) | Safe by default |
| `float` | `number` | Native |
| `decimal` | `string` (stripped `m`) | No native Decimal |
| `date` | `string` | No native date-only |
| `time` | `string` | No native time-only |
| `datetime` | `string` | Avoids `Date` object quirks |
| `uuid` | `string` | No native UUID |
| `binary` | `Uint8Array` | Native |
| `null`, `bool`, `string` | native | Native |

## Int64 Strategies

JS `number` safely represents integers only within `-(2^53 - 1)` to `2^53 - 1`. TTOON provides three optional modes (plus the default safe-error behavior):

### Default: Safe Error

```ts
import { parse } from '@ttoon/shared';

parse('9223372036854775807');  // throws: outside safe integer range
```

No silent precision loss. This is the safest default.

### BigInt Mode

```ts
import { use, intBigInt } from '@ttoon/shared';

await use({ int: intBigInt() });

const value = parse('9223372036854775807');
console.log(value);  // 9223372036854775807n (BigInt)
```

All integers become `BigInt`, including small ones.

### NaN Mode

```ts
import { use, intNumber } from '@ttoon/shared';

await use({ int: intNumber({ overflow: 'nan' }) });

const value = parse('9223372036854775807');
console.log(value);  // NaN
```

When overflow happens, `intNumber()` returns `NaN` instead of throwing.

### Lossy Mode

```ts
import { use, intNumber } from '@ttoon/shared';

await use({ int: intNumber({ overflow: 'lossy' }) });

const value = parse('9223372036854775807');
console.log(value);  // 9223372036854776000 (precision lost)
```

Explicitly accepts precision loss. Use only when you know your data fits or precision doesn't matter.

## Custom Codecs

### Decimal Codec Example

```ts
import Decimal from 'decimal.js';
import { use, type Codec } from '@ttoon/shared';

const decimalCodec: Codec<Decimal> = {
  type: 'decimal',
  fromPayload(payload) {
    if (typeof payload !== 'string') throw new Error('expected decimal payload');
    return new Decimal(payload.slice(0, -1));  // strip 'm' suffix
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

After registration:
```ts
const data = parse('price: 123.45m');
// data.price is now a Decimal instance, not a string
```

### Codec Interface

```ts
interface Codec<T> {
  type?: CodecType;                // 'int' | 'decimal' | 'date' | 'time' | 'datetime' | 'uuid' | 'binary'
  fromPayload(payload: CodecPayload): T;  // payload → JS value
  toPayload(value: T): CodecPayload;      // JS value → payload
  is(value: unknown): value is T;         // Type guard
}
```

### Registering Multiple Codecs

```ts
await use({
  int: intBigInt(),
  decimal: decimalCodec,
  date: dateCodec,
  uuid: uuidCodec,
});
```

### Per-Call Override

```ts
const data = parse(text, {
  codecs: { int: intBigInt() },  // override global for this call only
});
```

## Codec Scope

Codecs affect JS object-path value mapping in `parse()`, `stringify()`, `toTjson()`, and object-path streaming readers/writers. They do **not** affect:

- T-TOON / T-JSON syntax (the text format is always the same)
- Rust or Python type behavior
- Arrow schema inference (`readArrow()` / `stringifyArrow()`)
- Transcode operations (`tjsonToTtoon()` / `ttoonToTjson()`)

This is by design — codecs are a JS-specific adaptation layer, not a format-level feature.

## Streaming with Codecs

Object-path streaming readers and writers also respect codecs:

```ts
import { streamRead, StreamSchema, types, use, intBigInt } from '@ttoon/shared';

await use({ int: intBigInt() });

const schema = new StreamSchema({ id: types.int, name: types.string });

for await (const row of streamRead(source, { schema })) {
  console.log(typeof row.id);  // 'bigint'
}
```

Arrow streaming is **not** affected by codecs — Arrow uses native types.

## Recommendations

| Scenario | Strategy |
| :--- | :--- |
| Data with guaranteed small integers | Default (no codec) |
| Data with potential int64 values | `intBigInt()` |
| Financial/accounting data | Custom `decimal` codec with `decimal.js` or `big.js` |
| Date-heavy applications | Custom `date`/`time`/`datetime` codecs |
| Performance-critical Arrow pipelines | Skip codecs, use Arrow path |
