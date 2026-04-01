---
title: JavaScript / TypeScript 指南
sidebar_position: 2
sidebar_label: JS / TS
description: 使用 JavaScript 和 TypeScript 操作 TTOON 的完整指南 — 包含批次處理、Arrow、編解碼器、串流與轉碼。
---

# JavaScript / TypeScript 指南

`@ttoon/shared` 套件使用 WASM 橋接來呼叫 Rust 核心引擎以進行解析和序列化。JS 層添加了一個編解碼器 (codec) 系統來處理自訂型別對應 (因為 JS 缺乏原生的 `Decimal`, `UUID` 等型別)。

## 安裝

```bash
npm install @ttoon/shared

# 若要進行 Arrow 表格操作 (可選的 peer dependency)
npm install @ttoon/shared apache-arrow

# 針對自訂的十進位編解碼器 (請安裝您的編解碼器所使用的庫)
npm install @ttoon/shared decimal.js
npm install @ttoon/shared big.js
```

> `@ttoon/node` 和 `@ttoon/web` 是特定於環境的 `@ttoon/shared` 重新匯出包。除非您需要明確分離環境，否則請直接安裝 `@ttoon/shared`。

## 批次操作 (Batch Operations)

### 序列化 (Serialize)：`stringify()`

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

JS 缺乏原生的 `Decimal`, `UUID`, `Date` (只有日期) 和 `Time` 型別。在序列化期間，使用 `toon.*()` 標記來指示具型別值：

- `toon.uuid(str)` — UUID
- `toon.decimal(str)` — 帶有 `m` 後綴的十進位數 (Decimal)
- `toon.date(str)` — 日期 (`YYYY-MM-DD`)
- `toon.time(str)` — 時間 (`HH:MM:SS`)
- `toon.datetime(str)` — 日期時間 (ISO 8601)

**選項：**

| 參數 | 型別 | 預設值 | 說明 |
| :--- | :--- | :--- | :--- |
| `indentSize` | `number` | `2` | 縮排寬度 |
| `delimiter` | `string` | `','` | 表格分隔符：`','`, `'\t'`, 或是 `'|'` |
| `binaryFormat` | `string` | `'hex'` | 二進位編碼：`'hex'` 或是 `'b64'` |

### 反序列化 (Deserialize)：`parse()`

```ts
import { parse } from '@ttoon/shared';

const data = parse(text);
const strict = parse(text, { mode: 'strict' });
```

能自動偵測格式。預設的 JS 值對應：

| 具型別型別 | JS 結果 |
| :--- | :--- |
| `int` | `number` (超出安全範圍會拋出錯誤) |
| `float` | `number` |
| `decimal` | `string` (移除 `m` 後綴) |
| `date`, `time`, `datetime` | `string` |
| `uuid` | `string` |
| `binary` | `Uint8Array` |

若要更改這些預設值，請使用 `use()` 註冊編解碼器。

### 產生 T-JSON：`toTjson()`

```ts
import { toTjson, toon } from '@ttoon/shared';

const text = toTjson({
  id: toon.uuid('550e8400-e29b-41d4-a716-446655440000'),
  amount: toon.decimal('123.45'),
});
```

### 格式偵測：`detectFormat()`

```ts
import { detectFormat } from '@ttoon/shared';

detectFormat('{"key": 42}');       // 'tjson'
detectFormat('key: 42');           // 'typed_unit'
detectFormat('true');              // 'typed_unit'
```

## 直接轉碼 (Direct Transcode)

在不具現化 JS 物件的情況下轉換 T-JSON 和 T-TOON：

```ts
import { tjsonToTtoon, ttoonToTjson } from '@ttoon/shared';

const ttoonText = tjsonToTtoon('{"name": "Alice", "age": 30}');
const tjsonText = ttoonToTjson('name: "Alice"\nage: 30');

// 使用選項
const result = ttoonToTjson(text, { mode: 'strict', binaryFormat: 'b64' });
```

目前的 JS 直接轉碼包裝器無法區分單一 WASM 呼叫內部的解析或序列化錯誤。因此 `TranscodeError.phase` 總是回報為 `'parse'`；請使用 `sourceKind` 和底層的錯誤訊息來進行診斷。

## Arrow 路徑

需要可選的 peer dependency：`apache-arrow`。

```ts
import { readArrow, stringifyArrow, stringifyArrowTjson } from '@ttoon/shared';
import { tableFromArrays } from 'apache-arrow';

const table = tableFromArrays({
  name: ['Alice', 'Bob'],
  score: [95, 87],
});

// Arrow → T-TOON 表格
const ttoonText = await stringifyArrow(table);

// Arrow → T-JSON
const tjsonText = await stringifyArrowTjson(table);

// 文字 → Arrow
const restored = await readArrow(ttoonText);
```

Arrow 的 API 都是 `async`，因為它們會動態的載入 `apache-arrow` 模組。

## 編解碼器系統 (Codec System)

為了避免強制依賴第三方套件，JS 預設對 `decimal`, `date`, `time`, `datetime` 和 `uuid` 回傳字串。您可以註冊編解碼器來改變此行為：

```ts
import { use, intBigInt } from '@ttoon/shared';

// 將 int 切換為 BigInt 以確保 int64 的安全
await use({ int: intBigInt() });
```

### 內建的 Int 編解碼器

```ts
import { intNumber, intBigInt } from '@ttoon/shared';

// 預設行為：溢位時拋出錯誤 (安全)
await use({ int: intNumber() });

// 明確接受精度的損失
await use({ int: intNumber({ overflow: 'lossy' }) });

// 所有的整數都使用 BigInt
await use({ int: intBigInt() });
```

### 自訂編解碼器

```ts
import Decimal from 'decimal.js';
import { use, type Codec } from '@ttoon/shared';

const decimalCodec: Codec<Decimal> = {
  type: 'decimal',
  fromPayload(payload) {
    if (typeof payload !== 'string') throw new Error('預期的 decimal 負載 (payload)');
    return new Decimal(payload.slice(0, -1)); // 移除 'm' 後綴
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

編解碼器會影響 JS 在 `parse()`, `stringify()`, `toTjson()` 和物件路徑串流 API 中的值對映 (value mapping)。它們不會改變 T-TOON/T-JSON 的語法、Rust/Python 行為或 Arrow schema 的推論。

有關編解碼器和 int64 策略的更多資訊，請參見 [JS 編解碼器與 Int64](js-codecs-and-int64.md)。

## 串流 (Streaming)

若要進行逐行處理，請參見 [串流指南 (Streaming Guide)](streaming.md)。JS 串流讀取器回傳 `AsyncIterable`，寫入器則是基於推入 (push-based) 的類別：

```ts
import { streamRead, streamWriter, StreamSchema, types } from '@ttoon/shared';

const schema = new StreamSchema({ name: types.string, score: types.int });

// 讀取 (async iterable)
for await (const row of streamRead(source, { schema })) {
  console.log(row);
}

// 寫入 (push-based)
const writer = streamWriter(sink, { schema });
writer.write({ name: 'Alice', score: 95 });
await writer.close();
```

## 錯誤處理

```ts
import { ToonError, TranscodeError } from '@ttoon/shared';

try {
  parse(invalidText, { mode: 'strict' });
} catch (e) {
  if (e instanceof ToonError) {
    console.log(e.kind);    // 'parse' | 'serialize' | 'detect' | 'transcode'
    console.log(e.message); // 包含行/列詳細資訊
  }
}
```

對於 `tjsonToTtoon()` 和 `ttoonToTjson()`，請優先使用 `e.sourceKind` 和 `e.source.message` 而非 `e.phase`；目前的 JS 包裝器報告的總是 `phase: 'parse'`。

## 下一步

- **[JS 編解碼器與 Int64](js-codecs-and-int64.md)** — 深入了解編解碼器和 BigInt 的策略
- **[Arrow 與 Polars 指南](arrow-and-polars.md)** — 表格路徑詳細資訊
- **[串流指南](streaming.md)** — 逐行處理
- **[JS API 參考資料](../reference/js-api.md)** — 完整的 API 簽名
