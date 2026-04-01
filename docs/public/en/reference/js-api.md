---
title: JavaScript API Reference
sidebar_position: 3
sidebar_label: JS API
description: Complete JavaScript/TypeScript API reference for @ttoon/shared.
---

# JavaScript / TypeScript API Reference

Package: `@ttoon/shared` (npm)

## Batch APIs

### `parse<T>(text, options?): T`

Parse T-TOON / T-JSON / typed unit text to JS values.

**`ParseOptions`:**

| Property | Type | Default | Description |
| :--- | :--- | :--- | :--- |
| `mode` | `'compat' \| 'strict'` | `'compat'` | Parse mode (T-TOON / typed unit path only) |
| `codecs` | `CodecRegistry` | — | Per-call codec overrides |

### `stringify(value, options?): string`

Serialize JS values to T-TOON text.

- Uniform object lists auto-output as tabular `[N]{fields}:`

**`SerializeOptions`:**

| Property | Type | Default | Description |
| :--- | :--- | :--- | :--- |
| `indentSize` | `number` | `2` | Indentation width |
| `delimiter` | `',' \| '\t' \| '\|'` | `','` | Tabular separator |
| `binaryFormat` | `'hex' \| 'b64'` | `'hex'` | Binary encoding |

### `toTjson(value, options?): string`

Serialize JS values to T-JSON text.

**`TjsonSerializeOptions`:**

| Property | Type | Default | Description |
| :--- | :--- | :--- | :--- |
| `binaryFormat` | `'hex' \| 'b64'` | `'hex'` | Binary encoding |

### `detectFormat(text): Format`

Returns `'tjson'`, `'ttoon'`, or `'typed_unit'`.

## Type Markers

```ts
import { toon } from '@ttoon/shared';

toon.uuid('550e8400-...')     // UUID
toon.decimal('123.45')        // Decimal
toon.date('2026-03-08')       // Date
toon.time('14:30:00')         // Time
toon.datetime('2026-03-08T14:30:00+08:00')  // DateTime
```

## Transcode APIs

### `tjsonToTtoon(text, options?): string`

Convert T-JSON text directly to T-TOON text.

**`TjsonToTtoonOptions`:** extends `SerializeOptions`

### `ttoonToTjson(text, options?): string`

Convert T-TOON text directly to T-JSON text.

**`TtoonToTjsonOptions`:** extends `TjsonSerializeOptions` + `{ mode?: ParseMode }`

Note: for JS direct transcode, `TranscodeError.phase` is currently not reliable. The wrapper always reports `'parse'` because the underlying WASM call performs parse + serialize as one step.

## Arrow APIs (async)

Require the optional peer dependency `apache-arrow` when using Arrow APIs.

### `readArrow(text): Promise<ArrowTable>`

Parse text to Arrow Table. Auto-detects format. Input must be arrowable.

### `stringifyArrow(table, options?): Promise<string>`

Serialize Arrow Table to T-TOON tabular text.

### `stringifyArrowTjson(table, options?): Promise<string>`

Serialize Arrow Table to T-JSON (list-of-objects).

## Named Exports

These helpers and types are also exported from `@ttoon/shared`:

- `isArrowTable(value): boolean` — duck-type check for Arrow tables
- `IntPayload` — public int codec payload type
- `SourceErrorKind` — `'parse' | 'serialize' | 'detect'`
- `FieldTypeSpec` — schema field specification class
- `StreamSchemaInput` — `Record<string, FieldTypeSpec> | Iterable<[string, FieldTypeSpec]>`
- `ToonInput`, `ToonOutput`, `ToonTagged` — JS value surface types used by parse/stringify helpers

## Codec API

### `use(codecs): Promise<void>`

Register global codecs. Accepts a `CodecRegistry` object.

```ts
await use({ int: intBigInt(), decimal: myDecimalCodec });
```

### Built-in Int Codecs

```ts
import { intNumber, intBigInt } from '@ttoon/shared';

intNumber()                          // default: throw on overflow
intNumber({ overflow: 'nan' })       // return NaN on overflow
intNumber({ overflow: 'lossy' })     // accept precision loss
intBigInt()                          // all ints as BigInt
```

### `Codec<T>` Interface

```ts
interface Codec<T> {
  type?: CodecType;
  fromPayload(payload: CodecPayload): T;
  toPayload(value: T): CodecPayload;
  is(value: unknown): value is T;
}
```

`CodecRegistry` is `Partial<Record<CodecType, Codec>>`.

## Streaming APIs

### Factory Functions (Readers)

All readers return `AsyncIterable`:

| Function | Returns | Format |
| :--- | :--- | :--- |
| `streamRead(source, opts)` | `AsyncIterable<Record<string, unknown>>` | T-TOON |
| `streamReadTjson(source, opts)` | `AsyncIterable<Record<string, unknown>>` | T-JSON |
| `streamReadArrow(source, opts)` | `AsyncIterable<RecordBatch>` | T-TOON |
| `streamReadArrowTjson(source, opts)` | `AsyncIterable<RecordBatch>` | T-JSON |

**`StreamReadOptions`:**

| Property | Type | Required | Description |
| :--- | :--- | :--- | :--- |
| `schema` | `StreamSchema \| StreamSchemaInput` | Yes | Field definitions |
| `mode` | `ParseMode` | No | Parse mode |
| `codecs` | `CodecRegistry` | No | Codec overrides |

**`StreamReadArrowOptions`:**

| Property | Type | Required | Description |
| :--- | :--- | :--- | :--- |
| `schema` | `StreamSchema \| StreamSchemaInput` | Yes | Field definitions |
| `batchSize` | `number` | No | Rows per Arrow batch |
| `mode` | `ParseMode` | No | Parse mode |

### Factory Functions (Writers)

| Function | Returns | Format |
| :--- | :--- | :--- |
| `streamWriter(sink, opts)` | `StreamWriter` | T-TOON |
| `streamWriterTjson(sink, opts)` | `TjsonStreamWriter` | T-JSON |
| `streamWriterArrow(sink, opts)` | `ArrowStreamWriter` | T-TOON |
| `streamWriterArrowTjson(sink, opts)` | `TjsonArrowStreamWriter` | T-JSON |

**`StreamWriteOptions`:**

| Property | Type | Required | Description |
| :--- | :--- | :--- | :--- |
| `schema` | `StreamSchema \| StreamSchemaInput` | Yes | Field definitions |
| `delimiter` | `',' \| '\t' \| '\|'` | No | Tabular separator |
| `binaryFormat` | `'hex' \| 'b64'` | No | Binary encoding |
| `codecs` | `CodecRegistry` | No | Codec overrides |

**`StreamWriteArrowOptions`:**

| Property | Type | Required | Description |
| :--- | :--- | :--- | :--- |
| `schema` | `StreamSchema \| StreamSchemaInput` | Yes | Field definitions |
| `delimiter` | `',' \| '\t' \| '\|'` | No | Tabular separator |
| `binaryFormat` | `'hex' \| 'b64'` | No | Binary encoding |

**`TjsonStreamWriteOptions`:**

| Property | Type | Required | Description |
| :--- | :--- | :--- | :--- |
| `schema` | `StreamSchema \| StreamSchemaInput` | Yes | Field definitions |
| `binaryFormat` | `'hex' \| 'b64'` | No | Binary encoding |
| `codecs` | `CodecRegistry` | No | Codec overrides |

**`TjsonStreamWriteArrowOptions`:**

| Property | Type | Required | Description |
| :--- | :--- | :--- | :--- |
| `schema` | `StreamSchema \| StreamSchemaInput` | Yes | Field definitions |
| `binaryFormat` | `'hex' \| 'b64'` | No | Binary encoding |

### Writer Classes

| Class | Method | Input |
| :--- | :--- | :--- |
| `StreamWriter` | `write(row)` | `Record<string, unknown>` |
| `TjsonStreamWriter` | `write(row)` | `Record<string, unknown>` |
| `ArrowStreamWriter` | `writeBatch(batch)` | `RecordBatch` |
| `TjsonArrowStreamWriter` | `writeBatch(batch)` | `RecordBatch` |

All writers: `close(): Promise<StreamResult>`, `result: StreamResult | undefined`

### Source / Sink Types

**`TextSource`:** `string | Iterable<string | Uint8Array> | AsyncIterable<string | Uint8Array> | ReadableStreamLike<string | Uint8Array>`

**`TextSink`:** `((chunk: string) => void | Promise<void>) | { write(chunk: string): void | Promise<void> } | WritableStreamLike<string>`

### `StreamResult`

| Property | Type | Description |
| :--- | :--- | :--- |
| `rowsEmitted` | `number` | Number of rows written |

## Schema API

### `StreamSchema`

```ts
import { StreamSchema, types } from '@ttoon/shared';

const schema = new StreamSchema({
  name: types.string,
  score: types.int,
  amount: types.decimal(10, 2),
  active: types.bool.nullable(),
});

const schemaFromTuples = new StreamSchema([
  ['name', types.string],
  ['score', types.int],
]);
```

`StreamSchema` accepts `StreamSchemaInput = Record<string, FieldTypeSpec> | Iterable<[string, FieldTypeSpec]>`.

Validation rules:

- field names must be strings
- field values must be `FieldTypeSpec` instances built from `types`
- duplicate field names are rejected
- schemas must contain at least one field

**Methods:** `get(name)`, `entries()`, `keys()`, `values()`, `[Symbol.iterator]()`, `export()`

### `FieldTypeSpec`

Constructed via the `types` namespace. Supports `.nullable()` chaining.

### `types` Namespace

| Spec | Description |
| :--- | :--- |
| `types.string` | String |
| `types.int` | Integer |
| `types.float` | Float |
| `types.bool` | Boolean |
| `types.date` | Date |
| `types.time` | Time |
| `types.datetime` | DateTime (timezone-aware) |
| `types.datetimeNaive` | DateTime (naive) |
| `types.uuid` | UUID |
| `types.binary` | Binary |
| `types.decimal(precision, scale)` | Decimal |

## Error Types

### `ToonError`

| Property | Type | Description |
| :--- | :--- | :--- |
| `kind` | `ErrorKind` | `'parse' \| 'serialize' \| 'detect' \| 'transcode'` |
| `message` | `string` | Error message with line/column details |

### `TranscodeError` (extends `ToonError`)

| Property | Type | Description |
| :--- | :--- | :--- |
| `operation` | `TranscodeOperation` | `'tjson_to_ttoon' \| 'ttoon_to_tjson'` |
| `phase` | `TranscodePhase` | `'parse' \| 'serialize'`; for JS direct transcode it is currently always reported as `'parse'` |
| `sourceKind` | `SourceErrorKind` | Underlying source error kind |
| `source` | `ToonError` | Underlying source error |
