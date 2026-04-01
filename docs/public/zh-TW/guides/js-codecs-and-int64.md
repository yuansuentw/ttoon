---
title: JS 編解碼器與 Int64 (JS Codecs & Int64)
sidebar_position: 7
sidebar_label: JS 編解碼器與 Int64
description: JavaScript 中的自訂型別對應 — 包含編解碼器系統、int64 策略和 BigInt。
---

# JS 編解碼器與 Int64 (JS Codecs & Int64)

JavaScript 缺乏原生的 `Decimal`, `UUID`，以及僅包含日期 (date-only) 或僅包含時間 (time-only) 的值。TTOON 的編解碼器系統允許您自訂這些型別在 JS 中的表示方式。

## 預設行為 (無編解碼器)

| 具型別型別 | JS 結果 | 原因 |
| :--- | :--- | :--- |
| `int` | `number` (溢位則拋出錯誤) | 預設為安全機制 |
| `float` | `number` | 原生支援 |
| `decimal` | `string` (移除 `m`) | 無原生 Decimal |
| `date` | `string` | 無僅含日期的原生型別 |
| `time` | `string` | 無僅含時間的原生型別 |
| `datetime` | `string` | 避免 `Date` 物件的怪異行為 |
| `uuid` | `string` | 無原生 UUID |
| `binary` | `Uint8Array` | 原生支援 |
| `null`, `bool`, `string` | 原生對應型別 | 原生支援 |

## Int64 策略

JS 的 `number` 只能安全地表示介於 `-(2^53 - 1)` 到 `2^53 - 1` 之間的整數。TTOON 提供了三種可選模式（加上預設的安全錯誤行為）：

### 預設：安全的錯誤 (Safe Error)

```ts
import { parse } from '@ttoon/shared';

parse('9223372036854775807');  // 拋出錯誤: 超出安全整數範圍
```

不會有靜默的精確度流失。這是最安全的預設值。

### BigInt 模式

```ts
import { use, intBigInt } from '@ttoon/shared';

await use({ int: intBigInt() });

const value = parse('9223372036854775807');
console.log(value);  // 9223372036854775807n (BigInt)
```

所有的整數都會變成 `BigInt`，即使是小整數。

### NaN 模式

```ts
import { use, intNumber } from '@ttoon/shared';

await use({ int: intNumber({ overflow: 'nan' }) });

const value = parse('9223372036854775807');
console.log(value);  // NaN
```

發生溢位時，`intNumber()` 會回傳 `NaN` 而非拋出錯誤。

### 損失模式 (Lossy Mode)

```ts
import { use, intNumber } from '@ttoon/shared';

await use({ int: intNumber({ overflow: 'lossy' }) });

const value = parse('9223372036854775807');
console.log(value);  // 9223372036854776000 (精度遺失)
```

明確接受精確度的流失。請僅在您確定您的資料適用，或是不在意精確度時使用。

## 自訂編解碼器

### Decimal 編解碼器範例

```ts
import Decimal from 'decimal.js';
import { use, type Codec } from '@ttoon/shared';

const decimalCodec: Codec<Decimal> = {
  type: 'decimal',
  fromPayload(payload) {
    if (typeof payload !== 'string') throw new Error('預期的 decimal 負載 (payload)');
    return new Decimal(payload.slice(0, -1));  // 移除 'm' 後綴
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

註冊後：
```ts
const data = parse('price: 123.45m');
// data.price 現在是一個 Decimal 實例，而不是字串
```

### Codec 介面

```ts
interface Codec<T> {
  type?: CodecType;                // 'int' | 'decimal' | 'date' | 'time' | 'datetime' | 'uuid' | 'binary'
  fromPayload(payload: CodecPayload): T;  // payload → JS 值
  toPayload(value: T): CodecPayload;      // JS 值 → payload
  is(value: unknown): value is T;         // Type guard
}
```

### 註冊多個編解碼器

```ts
await use({
  int: intBigInt(),
  decimal: decimalCodec,
  date: dateCodec,
  uuid: uuidCodec,
});
```

### 單次呼叫的複寫 (Per-Call Override)

```ts
const data = parse(text, {
  codecs: { int: intBigInt() },  // 僅在這次呼叫中複寫全域設定
});
```

## 編解碼器的作用範圍 (Codec Scope)

編解碼器會在 `parse()`, `stringify()`, `toTjson()` 和物件路徑的串流讀寫器中影響 JS 的物件路徑值對應。它們**不會**影響：

- T-TOON / T-JSON 語法 (文字格式總是一樣的)
- Rust 或 Python 的型別行為
- Arrow Schema 推論 (`readArrow()` / `stringifyArrow()`)
- 轉碼操作 (`tjsonToTtoon()` / `ttoonToTjson()`)

這是刻意設計的 — 編解碼器是一個特定於 JS 的適配層，而不是格式等級的功能。

## 帶有編解碼器的串流 (Streaming with Codecs)

物件路徑的串流讀寫器同樣會遵循編解碼器：

```ts
import { streamRead, StreamSchema, types, use, intBigInt } from '@ttoon/shared';

await use({ int: intBigInt() });

const schema = new StreamSchema({ id: types.int, name: types.string });

for await (const row of streamRead(source, { schema })) {
  console.log(typeof row.id);  // 'bigint'
}
```

Arrow 串流**不會**受編解碼器影響 — Arrow 會使用其原生型別。

## 建議策略

| 情境 | 策略 |
| :--- | :--- |
| 保證是小整數的資料 | 預設 (不使用編解碼器) |
| 可能會有 int64 值的資料 | `intBigInt()` |
| 財務/會計資料 | 自訂搭配 `decimal.js` 或 `big.js` 的 `decimal` 編解碼器 |
| 偏重日期的應用程式 | 自訂的 `date`/`time`/`datetime` 編解碼器 |
| 對效能要求極高的 Arrow 流水線 | 略過編解碼器，直接使用 Arrow 路徑 |
