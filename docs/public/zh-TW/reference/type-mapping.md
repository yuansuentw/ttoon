---
title: 型別對應 (Type Mapping)
sidebar_position: 5
sidebar_label: 型別對應
description: 適用於 Python, JavaScript, Rust 和 Arrow 的跨語言型別轉換表。
---

# 型別對應 (Type Mapping)

本頁說明 TTOON `typed types` 如何在不同語言、Arrow，以及主流 RDBMS 之間對應。

## 設計原則

TTOON 的 `typed types` 是跨系統交換層，不是任一資料庫型別系統的完整鏡像。它刻意選擇 PostgreSQL、MySQL / MariaDB、SQLite、SQL Server 之間最常用、最容易穩定映射的一組通用型別。

這表示：

- TTOON 關注的是跨語言與跨資料庫的共同語意，不是保留所有 vendor-specific 細節
- 同一個 typed type 在不同資料庫中，可能對應到不同實體欄位型別
- SQLite 採用動態型別系統，因此多數映射是「慣例儲存形式」，不是強型別欄位宣告

## 主流 RDBMS 映射

| TTOON typed type | 代表語意 | PostgreSQL | MySQL / MariaDB | SQLite | SQL Server | 備註 |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| `null` | 空值 | `NULL` | `NULL` | `NULL` | `NULL` | 所有資料庫通用 |
| `bool` | 布林旗標 | `boolean` | `boolean` / `tinyint(1)` | `INTEGER` 0/1 或 `BOOLEAN` 別名 | `bit` | SQLite 無獨立布林儲存類別 |
| `int` | 有號整數 | `smallint` / `integer` / `bigint` | `tinyint` / `smallint` / `int` / `bigint` | `INTEGER` | `smallint` / `int` / `bigint` | TTOON 不內建 unsigned-only 語意 |
| `float` | 近似浮點數 | `real` / `double precision` | `float` / `double` | `REAL` | `real` / `float` | 適用近似數值，不保證十進位精度 |
| `decimal` | 精確十進位 | `numeric` / `decimal` | `decimal` / `numeric` | 通常以 `NUMERIC` 慣例、`TEXT` 或應用層 decimal 處理 | `decimal` / `numeric` | SQLite 無固定精度 decimal 儲存類別 |
| `string` | 文字 | `text` / `varchar` | `char` / `varchar` / `text` | `TEXT` | `nvarchar` / `varchar` / `nchar` / `char` | TTOON canonical form 為 UTF-8 字串 |
| `date` | 日期 | `date` | `date` | 常用 `TEXT` (`YYYY-MM-DD`) | `date` | SQLite 為慣例表示法 |
| `time` | 純時間 | `time` | `time` | 常用 `TEXT` (`HH:MM:SS[.ffffff]`) | `time` | 不含日期；時區能力依來源系統而異 |
| `datetime` | 日期時間 / 時間戳 | `timestamp` / `timestamptz` | `datetime` / `timestamp` | 常用 ISO 8601 `TEXT` 或 Unix time `INTEGER` | `datetime2` / `datetimeoffset` | 若來源型別支援時區，TTOON 會保留 |
| `uuid` | 128-bit 識別值 | `uuid` | 常見為 `binary(16)` 或 `char(36)` | 常見為 `TEXT` 或 `BLOB` | `uniqueidentifier` | MySQL / SQLite 無通用原生 UUID 欄位 |
| `hex` | 二進位資料 | `bytea` | `binary` / `varbinary` / `blob` | `BLOB` | `binary` / `varbinary` | 僅文字表示採十六進位 |
| `b64` | 二進位資料 | `bytea` | `binary` / `varbinary` / `blob` | `BLOB` | `binary` / `varbinary` | 僅文字表示採 Base64 |

## 物件路徑 (Object Path)

| 具型別型別 | Python `loads()` | JS `parse()` 預設 | JS 使用編解碼器 (Codec) |
| :--- | :--- | :--- | :--- |
| `null` | `None` | `null` | — |
| `bool` | `bool` | `boolean` | — |
| `int` | `int` | `number` (如果不安全會拋出錯誤) | 透過 `intBigInt()` 轉換為 `bigint` |
| `float` | `float` | `number` | — |
| `decimal` | `decimal.Decimal` | `string` (移除 `m`) | 透過編解碼器轉換為 `Decimal` 類別 |
| `string` | `str` | `string` | — |
| `date` | `datetime.date` | `string` | 透過自訂編解碼器轉換 |
| `time` | `datetime.time` | `string` | 透過自訂編解碼器轉換 |
| `datetime` | `datetime.datetime` | `string` | 透過自訂編解碼器轉換 |
| `uuid` | `uuid.UUID` | `string` | 透過自訂編解碼器轉換 |
| `hex`/`b64` | `bytes` | `Uint8Array` | 透過自訂編解碼器轉換 |

對於 JS 的序列化輸入，例如 `bigint`、`Date`、陣列和純物件，請參考下方的「序列化方向」區塊。

### 為什麼 JS 在某些型別上會回傳字串

JS 沒有原生的 `Decimal`、`UUID`、僅有日期的 (date-only) 或僅有時間的 (time-only) 型別。預設回傳字串可以避免：

- 強制依賴第三方函式庫 (`decimal.js`, `moment.js`)
- 瀏覽器和 Node.js 之間在執行時產生的假設分歧

使用 `use()` 註冊編解碼器以支援更豐富的型別。請參見 [JS 編解碼器與 Int64](../guides/js-codecs-and-int64.md)。

## Arrow 路徑

| 具型別型別 | Python `read_arrow()` | JS `readArrow()` | Arrow 原生型別 |
| :--- | :--- | :--- | :--- |
| `null` | 允許為 null 的欄位 (Nullable column) | 允許為 null 的欄位 | `Null` 或可為 null 的具型別欄位 |
| `bool` | `Boolean` | `Bool` | `Boolean` |
| `int` | `Int64` | `Int64` | `Int64` |
| `float` | `Float64` | `Float64` | `Float64` |
| `decimal` | `Decimal128/256` | `Decimal` | `Decimal128` 或 `Decimal256` (取決於精度) |
| `string` | `Utf8` | `Utf8` | `Utf8` |
| `date` | `Date32` | `DateDay` | `Date32` |
| `time` | `Time64(Microsecond)` | `TimeMicrosecond` | `Time64(Microsecond)` |
| `datetime` | `Timestamp(μs[, tz])` | `TimestampMicrosecond` | `Timestamp(Microsecond[, tz])` |
| `uuid` | `FixedSizeBinary(16)` | `FixedSizeBinary(16)` | `FixedSizeBinary(16)` + UUID 表徵資料 |
| `hex`/`b64` | `Binary` | `Binary` | `Binary` / `LargeBinary` / `FixedSizeBinary` |

### 讀表與寫表時的實務對照

- `decimal` 對應的資料庫欄位通常應優先保留為 exact numeric，而不是轉成浮點數
- `uuid` 在 PostgreSQL / SQL Server 可直用原生型別；MySQL / SQLite 常需靠 schema 慣例決定 `binary(16)` 或字串格式
- `date` / `time` / `datetime` 在 SQLite 幾乎總是應用層約定，不應假設有與 PostgreSQL 相同的強型別保證
- `hex` 與 `b64` 在資料庫層通常都回到同一種 binary 欄位；兩者差異只存在於 TTOON 文字編碼

### Arrow 型別的保留

Arrow 型別會以其原生的解析度被保存：

- `decimal` 使用 `Decimal128` 或 `Decimal256` — 絕不會降級為 `Utf8`
- `uuid` 使用附帶 UUID 表徵資料的 `FixedSizeBinary(16)` — 絕不會降級為 `Utf8`
- `datetime` 的時區資訊在存在時會被保留下來
- 數值全部皆為 null 的 `null` 欄位會被推論為 `Null` 類型

### Datetime 時區行為

- 包含時區與不包含時區的 datetimes 不能在同一個欄位 (column) 內混用
- JS Arrow 橋接器強制執行這個規定，混用時會觸發 Schema 推論錯誤
- 包含時區的 `datetime` 使用 `Timestamp(Microsecond, tz)`
- 不包含時區 (naive) 的 `datetime` 使用 `Timestamp(Microsecond)`

## 序列化方向

| Python 型別 | 具型別輸出 (Typed Output) |
| :--- | :--- |
| `None` | `null` |
| `bool` | `true` / `false` |
| `int` | `42` |
| `float` | `3.14` |
| `decimal.Decimal` | `123.45m` |
| `str` | `"Alice"` |
| `datetime.date` | `2026-03-08` |
| `datetime.time` | `14:30:00` |
| `datetime.datetime` | `2026-03-08T14:30:00` |
| `uuid.UUID` | `uuid(550e8400-...)` |
| `bytes` | `hex(...)` 或 `b64(...)` |

| JS 值 | 具型別輸出 (Typed Output) |
| :--- | :--- |
| `null` | `null` |
| `boolean` | `true` / `false` |
| `number` (安全的整數) | `42` |
| `number` (浮點數) | `3.14` |
| `string` | `"Alice"` |
| `bigint` (有號 i64 範圍內) | `42` |
| `Date` | `2026-03-08T14:30:00Z` |
| `Array` | `[1, 2]` |
| `純物件 (plain object)` | `{"name": "Alice"}` |
| `Uint8Array` | `hex(...)` 或 `b64(...)` |
| `toon.decimal('123.45')` | `123.45m` |
| `toon.uuid('...')` | `uuid(...)` |
| `toon.date('...')` | `2026-03-08` |
| `toon.time('14:30:00')` | `14:30:00` |
| `toon.datetime('2026-03-08T14:30:00+08:00')` | `2026-03-08T14:30:00+08:00` |
