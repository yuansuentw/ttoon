---
title: JavaScript API 參考資料 (JavaScript API Reference)
sidebar_position: 3
sidebar_label: JS API
description: @ttoon/shared 的完整 JavaScript/TypeScript API 參考資料。
---

# JavaScript / TypeScript API 參考資料

套件：`@ttoon/shared` (npm)

## 批次 API (Batch APIs)

### `parse<T>(text, options?): T`

將 T-TOON / T-JSON / 具型別單位文字解析為 JS 的值。

**`ParseOptions`:**

| 屬性 | 型別 | 預設值 | 描述 |
| :--- | :--- | :--- | :--- |
| `mode` | `'compat' \| 'strict'` | `'compat'` | 解析模式 (僅用於 T-TOON / 具型別單位路徑) |
| `codecs` | `CodecRegistry` | — | 於該次呼叫中覆寫編解碼器 (codec overrides) |

### `stringify(value, options?): string`

將 JS 值序列化為 T-TOON 文字。

- 統一物件清單會自動輸出為表格標頭 `[N]{fields}:`

**`SerializeOptions`:**

| 屬性 | 型別 | 預設值 | 描述 |
| :--- | :--- | :--- | :--- |
| `indentSize` | `number` | `2` | 縮排寬度 |
| `delimiter` | `',' \| '\t' \| '\|'` | `','` | 表格分隔符 |
| `binaryFormat` | `'hex' \| 'b64'` | `'hex'` | 二進位編碼 |

### `toTjson(value, options?): string`

將 JS 值序列化為 T-JSON 文字。

**`TjsonSerializeOptions`:**

| 屬性 | 型別 | 預設值 | 描述 |
| :--- | :--- | :--- | :--- |
| `binaryFormat` | `'hex' \| 'b64'` | `'hex'` | 二進位編碼 |

### `detectFormat(text): Format`

回傳 `'tjson'`, `'ttoon'`, 或是 `'typed_unit'`。

## 類型標記 (Type Markers)

```ts
import { toon } from '@ttoon/shared';

toon.uuid('550e8400-...')     // UUID
toon.decimal('123.45')        // 十進位
toon.date('2026-03-08')       // 日期
toon.time('14:30:00')         // 時間
toon.datetime('2026-03-08T14:30:00+08:00')  // 日期時間
```

## 轉碼 API (Transcode APIs)

### `tjsonToTtoon(text, options?): string`

直接將 T-JSON 文字轉換為 T-TOON 文字。

**`TjsonToTtoonOptions`:** 繼承自 `SerializeOptions`

### `ttoonToTjson(text, options?): string`

直接將 T-TOON 文字轉換為 T-JSON 文字。

**`TtoonToTjsonOptions`:** 繼承自 `TjsonSerializeOptions` + `{ mode?: ParseMode }`

備註：就 JS 上的直接轉碼來說，`TranscodeError.phase` 目前並不十分可靠。這個包裝函數將總是報告回傳 `'parse'`，因為底層 WASM 調用在一個執行步驟裡同時執行了解析和序列化。

## Arrow APIs (async 非同步操作)

當要使用 Arrow 相關 API 時，需要有可選的 peer dependency：`apache-arrow`。

### `readArrow(text): Promise<ArrowTable>`

將文字解析層 Arrow Table。自動偵測格式。輸入的值必須支援轉換為 Arrow  (arrowable)。

### `stringifyArrow(table, options?): Promise<string>`

將 Arrow Table 序列化為 T-TOON 的表格文字。

### `stringifyArrowTjson(table, options?): Promise<string>`

將 Arrow Table 序列化為 T-JSON 格式 (物件清單)。

## 具名匯出 (Named Exports)

下列助手函式與類型也一併由 `@ttoon/shared` 匯出：

- `isArrowTable(value): boolean` — 使用鴨子型別 (duck-type) 的方式檢查是否為 Arrow 表格
- `IntPayload` — 公開的 int 編碼有效負載 (payload) 型別
- `SourceErrorKind` — `'parse' | 'serialize' | 'detect'`
- `FieldTypeSpec` — Schema 欄位規格類別
- `StreamSchemaInput` — `Record<string, FieldTypeSpec> | Iterable<[string, FieldTypeSpec]>`
- `ToonInput`, `ToonOutput`, `ToonTagged` — 於 parse/stringify 助手函式使用的 JS 的值或表面類型

## 編解碼器 API (Codec API)

### `use(codecs): Promise<void>`

註冊全域的編解碼器 (global codecs)。接受一個 `CodecRegistry` 物件。

```ts
await use({ int: intBigInt(), decimal: myDecimalCodec });
```

### 內建的 Int 編解碼器

```ts
import { intNumber, intBigInt } from '@ttoon/shared';

intNumber()                          // default: 若溢位則拋出錯誤
intNumber({ overflow: 'nan' })       // 若溢位，回傳 NaN 
intNumber({ overflow: 'lossy' })     // 可接受精確度的流失
intBigInt()                          // 所有的 ints 均轉變為 BigInt 格式
```

### `Codec<T>` 介面

```ts
interface Codec<T> {
  type?: CodecType;
  fromPayload(payload: CodecPayload): T;
  toPayload(value: T): CodecPayload;
  is(value: unknown): value is T;
}
```

`CodecRegistry` 即是 `Partial<Record<CodecType, Codec>>`。

## 串流 API (Streaming APIs)

### 工廠函式 (讀取器 Reader)

所有的讀取器皆回傳了 `AsyncIterable`：

| 函數 (Function) | 回傳型別 (Returns) | 格式 (Format) |
| :--- | :--- | :--- |
| `streamRead(source, opts)` | `AsyncIterable<Record<string, unknown>>` | T-TOON |
| `streamReadTjson(source, opts)` | `AsyncIterable<Record<string, unknown>>` | T-JSON |
| `streamReadArrow(source, opts)` | `AsyncIterable<RecordBatch>` | T-TOON |
| `streamReadArrowTjson(source, opts)` | `AsyncIterable<RecordBatch>` | T-JSON |

**`StreamReadOptions`:**

| 屬性 | 型別 | 必填 | 描述 |
| :--- | :--- | :--- | :--- |
| `schema` | `StreamSchema \| StreamSchemaInput` | 是 | 欄位定義 |
| `mode` | `ParseMode` | 否 | 解析模式 |
| `codecs` | `CodecRegistry` | 否 | 當次要覆寫的編解碼器 |

**`StreamReadArrowOptions`:**

| 屬性 | 型別 | 必填 | 描述 |
| :--- | :--- | :--- | :--- |
| `schema` | `StreamSchema \| StreamSchemaInput` | 是 | 欄位定義 |
| `batchSize` | `number` | 否 | 每一個 Arrow 批次的行數 |
| `mode` | `ParseMode` | 否 | 解析模式 |

### 工廠函數 (寫入器 Writer)

| 函數 (Function) | 回傳型別 (Returns) | 格式 (Format) |
| :--- | :--- | :--- |
| `streamWriter(sink, opts)` | `StreamWriter` | T-TOON |
| `streamWriterTjson(sink, opts)` | `TjsonStreamWriter` | T-JSON |
| `streamWriterArrow(sink, opts)` | `ArrowStreamWriter` | T-TOON |
| `streamWriterArrowTjson(sink, opts)` | `TjsonArrowStreamWriter` | T-JSON |

**`StreamWriteOptions`:**

| 屬性 | 型別 | 必填 | 描述 |
| :--- | :--- | :--- | :--- |
| `schema` | `StreamSchema \| StreamSchemaInput` | 是 | 欄位定義 |
| `delimiter` | `',' \| '\t' \| '\|'` | 否 | 表格的分隔符號 |
| `binaryFormat` | `'hex' \| 'b64'` | 否 | 二進位編碼格式 |
| `codecs` | `CodecRegistry` | 否 | 當次要覆寫的編解碼器 |

**`StreamWriteArrowOptions`:**

| 屬性 | 型別 | 必填 | 描述 |
| :--- | :--- | :--- | :--- |
| `schema` | `StreamSchema \| StreamSchemaInput` | 是 | 欄位定義 |
| `delimiter` | `',' \| '\t' \| '\|'` | 否 | 表格的分隔符號 |
| `binaryFormat` | `'hex' \| 'b64'` | 否 | 二進位編碼格式 |

**`TjsonStreamWriteOptions`:**

| 屬性 | 型別 | 必填 | 描述 |
| :--- | :--- | :--- | :--- |
| `schema` | `StreamSchema \| StreamSchemaInput` | 是 | 欄位定義 |
| `binaryFormat` | `'hex' \| 'b64'` | 否 | 二進位編碼格式 |
| `codecs` | `CodecRegistry` | 否 | 當次要覆寫的編解碼器 |

**`TjsonStreamWriteArrowOptions`:**

| 屬性 | 型別 | 必填 | 描述 |
| :--- | :--- | :--- | :--- |
| `schema` | `StreamSchema \| StreamSchemaInput` | 是 | 欄位定義 |
| `binaryFormat` | `'hex' \| 'b64'` | 否 | 二進位編碼 |

### 寫入器類別 (Writer Classes)

| 類別 (Class) | 寫入方法 (Method) | 需求輸入 (Input) |
| :--- | :--- | :--- |
| `StreamWriter` | `write(row)` | `Record<string, unknown>` |
| `TjsonStreamWriter` | `write(row)` | `Record<string, unknown>` |
| `ArrowStreamWriter` | `writeBatch(batch)` | `RecordBatch` |
| `TjsonArrowStreamWriter` | `writeBatch(batch)` | `RecordBatch` |

在所有的寫入器中包含的屬性或函式：`close(): Promise<StreamResult>` 以及 `result: StreamResult | undefined` （這會在成功執行了函式 `close()` 後才有有效值）

### 來源 (Source) / 寫入目的地 (Sink) 型別

**`TextSource`:** `string | Iterable<string | Uint8Array> | AsyncIterable<string | Uint8Array> | ReadableStreamLike<string | Uint8Array>`

**`TextSink`:** `((chunk: string) => void | Promise<void>) | { write(chunk: string): void | Promise<void> } | WritableStreamLike<string>`

### `StreamResult`

| 屬性 | 型別 | 描述 |
| :--- | :--- | :--- |
| `rowsEmitted` | `number` | 當前成功寫入的全部總行數 |

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

`StreamSchema` 可以接受傳入 `StreamSchemaInput = Record<string, FieldTypeSpec> | Iterable<[string, FieldTypeSpec]>` 作為參數建構。

有效性驗證規則：

- 欄位名稱必須為 string字串
- field 欄位數值必須是由 `types` 建構而成的 `FieldTypeSpec` 實例。
- 不接受重複的鍵詞 (或者是名稱)
- 所有的 schema 至少都需要帶有一種欄位

**支援的方法函式：** `get(name)`, `entries()`, `keys()`, `values()`, `[Symbol.iterator]()`, `export()`

### `FieldTypeSpec`

透過命名空間 `types` 所建構的型別實例。可以與接續的 `.nullable()` 串流鍊（chaining）共用搭配。

### `types` 命名空間 (Namespace)

| 型別規格 (Spec) | 描述 |
| :--- | :--- |
| `types.string` | 字串 |
| `types.int` | 整數 |
| `types.float` | 浮點數 |
| `types.bool` | 布林值 |
| `types.date` | 日期 |
| `types.time` | 時間 |
| `types.datetime` | 日期時間 (能意識並考慮時區的作用) |
| `types.datetimeNaive` | 日期時間 (未能認知時區的存在) |
| `types.uuid` | UUID |
| `types.binary` | 二進位 (Binary) |
| `types.decimal(precision, scale)` | 十進位數 (Decimal) |

## 錯誤種類型 (Error Types)

### `ToonError`

| 屬性 | 型別 | 描述 |
| :--- | :--- | :--- |
| `kind` | `ErrorKind` | `'parse' \| 'serialize' \| 'detect' \| 'transcode'` |
| `message` | `string` | 內含錯誤行/列細節內容的錯誤訊息字串 |

### `TranscodeError` (自 `ToonError` 中所衍生)

| 屬性 | 型別 | 描述 |
| :--- | :--- | :--- |
| `operation` | `TranscodeOperation` | `'tjson_to_ttoon' \| 'ttoon_to_tjson'` |
| `phase` | `TranscodePhase` | `'parse' \| 'serialize'`; 而目前因為在 JS 架構的問題所以將永遠只會回報自己是在 `'parse'` 所發生。 |
| `sourceKind` | `SourceErrorKind` | 本身的發生來源是何種錯誤？ |
| `source` | `ToonError` | 有關發生的原始來源錯誤為何？ |
