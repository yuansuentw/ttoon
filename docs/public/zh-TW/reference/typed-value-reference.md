---
title: typed value 參考
sidebar_position: 2
sidebar_label: typed value 參考
description: TTOON typed value、執行期對應、Arrow 對應與 RDBMS 對照的完整參考。
---

# typed value 參考

本頁是 TTOON typed value layer 的詳細參考。如果你只想先快速理解兩種語法與基本規則，請先看 [格式總覽](../getting-started/format-overview.md)。

在結構層面上，T-TOON 是建立在 [TOON](https://toonformat.dev/) 與其 [toon-format 專案](https://github.com/toon-format/toon) 之上的擴充。本頁聚焦的則是 TTOON 在這個基礎上額外加入的 typed value layer 與跨語言語意。

## 術語

- **typed**：值的文字編碼本身直接攜帶型別語意的整體設計概念
- **typed types**：12 種內建值編碼
- **typed unit**：解析器直接看到的一段單一序列化值，例如 `123.45m`、`2026-03-08`、`uuid(...)`
- `hex` 與 `b64` 都代表 binary payload，但在文字格式層仍各自算一種 typed type

## typed value 系統

兩種語法共用同一組 12 種 typed value 編碼：

| 型別 | 語法 | 範例 | 備註 |
| :--- | :--- | :--- | :--- |
| `null` | 關鍵字 | `null` | Null 值 |
| `bool` | 關鍵字 | `true`, `false` | 僅限小寫 |
| `int` | 數字，可帶正負號與 `_` 分隔符 | `42`, `-1_000`, `0` | 允許正負號與 `_` |
| `float` | 小數點或科學記號 | `3.14`, `1e-9`, `-0.5`, `inf`, `-inf`, `nan` | 特殊值：`inf`、`-inf`、`nan` |
| `decimal` | 帶 `m` 後綴的數字 | `123.45m`, `0.00m`, `-99.9m` | 小寫 `m`；不使用科學記號 |
| `string` | 雙引號包覆 | `"Alice"`, `"hello\nworld"` | 一律使用雙引號 |
| `date` | `YYYY-MM-DD` | `2026-03-08` | 在兩種語法中都可裸寫 |
| `time` | `HH:MM:SS[.fractional]` | `14:30:00`, `14:30:00.123456` | 最多到微秒 |
| `datetime` | ISO 8601 | `2026-03-08T14:30:00`, `2026-03-08T14:30:00+08:00`, `2026-03-08T14:30:00Z` | 可含時區 |
| `uuid` | `uuid(...)` 包裝器 | `uuid(550e8400-e29b-41d4-a716-446655440000)` | 僅接受小寫 hex |
| `hex` | `hex(...)` 包裝器 | `hex(4A42)`, `hex(48656C6C6F)` | 十六進位 binary 編碼 |
| `b64` | `b64(...)` 包裝器 | `b64(SkI=)`, `b64(SGVsbG8=)` | Base64 binary 編碼 |

### 為何是這 12 種 typed type

TTOON 的 typed types 並不是照抄某一種資料庫或語言型別系統，而是取主流 RDBMS、Arrow，以及 Python / JS / Rust SDK 之間最實用的交集。

設計目標是讓型別層：

- 足以覆蓋常見的跨系統資料交換
- 在 object path 與 Arrow path 都能保持穩定語意
- 不被單一資料庫私有型別綁死

因此像 `jsonb`、`enum`、`interval`、`array`、地理型別、`money` 這類型別，暫時不作為內建 typed type，而應交由上層 schema 慣例或自訂 codec 處理。

### 關鍵規則

- **字串一定加引號**：所有字串都用雙引號 `"..."`，避免 bare token 歧義
- **Decimal 用 `m` 後綴**：`123.45m` 是 exact decimal；`123.45` 是 float
- **UUID 與 binary 用包裝器**：`uuid(...)`、`hex(...)`、`b64(...)`

## 型別細節

### null

`null` 是單一關鍵字，表示沒有值。

### bool

布林值只有 `true` 與 `false` 兩種形式，而且必須是小寫。

### int

整數允許正負號與 `_` 數字分隔符：

```text
42
-1_000_000
0
```

JS 精度注意事項：JS 原生 `number` 只能安全表示 `-(2^53 - 1)` 到 `2^53 - 1` 之間的整數。超出此範圍預設會拋錯。需要時請改用 `intBigInt()` 或 `intNumber({ overflow: 'lossy' })`。詳見 [JS codec 與 Int64](../guides/js-codecs-and-int64.md)。

### float

浮點數必須包含小數點或使用科學記號：

```text
3.14
1e-9
-0.5
inf
-inf
nan
```

### decimal

精確十進位使用小寫 `m` 後綴：

```text
123.45m
0.00m
-99.9m
```

Decimal 不使用科學記號。`123.45m` 是 decimal；`123.45` 是 float。

### string

字串一律使用雙引號：

```text
"Alice"
"hello world"
""
```

escape 規則：

- **T-TOON**：只允許 `\\`、`\"`、`\n`、`\r`、`\t`
- **T-JSON**：採完整 JSON escape 集合，包含 `\uXXXX`、`\b`、`\f`

### date

格式為 `YYYY-MM-DD`，在兩種語法中都可裸寫：

```text
2026-03-08
```

### time

格式為 `HH:MM:SS`，可帶最多微秒精度的小數秒：

```text
14:30:00
14:30:00.123456
```

### datetime

採 ISO 8601，可選時區：

```text
2026-03-08T14:30:00
2026-03-08T14:30:00+08:00
2026-03-08T14:30:00Z
```

### uuid

UUID 使用 `uuid(...)`：

```text
uuid(550e8400-e29b-41d4-a716-446655440000)
```

必須符合標準 8-4-4-4-12 形狀，且只接受小寫 hex。

### hex / b64

Binary payload 可使用十六進位或 Base64：

```text
hex(48656C6C6F)
b64(SGVsbG8=)
```

`hex` 與 `b64` 在執行期都對應 binary data；差異只存在於文字編碼形式。

## 跨語言對應（Object Path）

| typed type | Python `loads()` | JS `parse()` 預設 | JS 使用 Codec |
| :--- | :--- | :--- | :--- |
| `null` | `None` | `null` | — |
| `bool` | `bool` | `boolean` | — |
| `int` | `int` | `number`（不安全時會拋錯） | 透過 `intBigInt()` 轉為 `bigint` |
| `float` | `float` | `number` | — |
| `decimal` | `decimal.Decimal` | `string`（去掉 `m`） | 透過 codec 轉為 `Decimal` 類別 |
| `string` | `str` | `string` | — |
| `date` | `datetime.date` | `string` | 自訂 codec |
| `time` | `datetime.time` | `string` | 自訂 codec |
| `datetime` | `datetime.datetime` | `string` | 自訂 codec |
| `uuid` | `uuid.UUID` | `string` | 自訂 codec |
| `hex`/`b64` | `bytes` | `Uint8Array` | 自訂 codec |

### 為什麼 JS 某些型別預設回傳字串

JS 沒有原生 `Decimal`、`UUID`、date-only、time-only 型別。預設回傳字串可以避免：

- 強制依賴第三方套件
- Browser 與 Node.js 執行期假設分歧

需要更豐富型別時，請用 `use()` 註冊 codec。詳見 [JS codec 與 Int64](../guides/js-codecs-and-int64.md)。

## Arrow 路徑

| typed type | Python `read_arrow()` | JS `readArrow()` | Arrow 原生型別 |
| :--- | :--- | :--- | :--- |
| `null` | 可為 null 的欄位 | 可為 null 的欄位 | `Null` 或可為 null 的 typed column |
| `bool` | `Boolean` | `Bool` | `Boolean` |
| `int` | `Int64` | `Int64` | `Int64` |
| `float` | `Float64` | `Float64` | `Float64` |
| `decimal` | `Decimal128/256` | `Decimal` | `Decimal128` 或 `Decimal256` |
| `string` | `Utf8` | `Utf8` | `Utf8` |
| `date` | `Date32` | `DateDay` | `Date32` |
| `time` | `Time64(Microsecond)` | `TimeMicrosecond` | `Time64(Microsecond)` |
| `datetime` | `Timestamp(μs[, tz])` | `TimestampMicrosecond` | `Timestamp(Microsecond[, tz])` |
| `uuid` | `FixedSizeBinary(16)` | `FixedSizeBinary(16)` | `FixedSizeBinary(16)` + UUID metadata |
| `hex`/`b64` | `Binary` | `Binary` | `Binary` / `LargeBinary` / `FixedSizeBinary` |

### Arrow 型別保留

- `decimal` 會用 `Decimal128` 或 `Decimal256`，不會降成 `Utf8`
- `uuid` 會用帶 UUID metadata 的 `FixedSizeBinary(16)`，不會降成 `Utf8`
- `datetime` 若有時區會保留時區資訊
- 全 null 欄位會推論成 `Null`

### Datetime 時區行為

- 同一欄位不能混用有時區與無時區的 datetime
- JS Arrow bridge 會強制這條規則，混用時回報 schema inference error
- 有時區的 `datetime` 使用 `Timestamp(Microsecond, tz)`
- 無時區的 `datetime` 使用 `Timestamp(Microsecond)`

### 讀寫實務建議

- `decimal` 通常應保留為 exact numeric 欄位，而不是降成 float
- `uuid` 在 PostgreSQL 與 SQL Server 可直接用原生型別；MySQL / SQLite 常用 `binary(16)` 或字串慣例
- `date` / `time` / `datetime` 在 SQLite 多半只是應用層慣例，不是強型別保證
- `hex` 與 `b64` 在資料庫層通常都回到同一種 binary 欄位型別

## 主流 RDBMS 對照

| TTOON typed type | 語意 | PostgreSQL | MySQL / MariaDB | SQLite | SQL Server | 備註 |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| `null` | 空值 | `NULL` | `NULL` | `NULL` | `NULL` | 通用 |
| `bool` | 布林旗標 | `boolean` | `boolean` / `tinyint(1)` | `INTEGER` 0/1 或 `BOOLEAN` 別名 | `bit` | SQLite 無獨立 boolean storage class |
| `int` | 有號整數 | `smallint` / `integer` / `bigint` | `tinyint` / `smallint` / `int` / `bigint` | `INTEGER` | `smallint` / `int` / `bigint` | TTOON 不定義 unsigned-only 語意 |
| `float` | 近似數值 | `real` / `double precision` | `float` / `double` | `REAL` | `real` / `float` | 非精確十進位 |
| `decimal` | 精確十進位 | `numeric` / `decimal` | `decimal` / `numeric` | 常見為 `NUMERIC`、`TEXT` 或應用層 decimal | `decimal` / `numeric` | SQLite 無固定精度 decimal storage class |
| `string` | 文字 | `text` / `varchar` | `char` / `varchar` / `text` | `TEXT` | `nvarchar` / `varchar` / `nchar` / `char` | TTOON canonical form 為 UTF-8 文字 |
| `date` | 日期 | `date` | `date` | 常見為 `TEXT` (`YYYY-MM-DD`) | `date` | SQLite 以慣例為主 |
| `time` | 純時間 | `time` | `time` | 常見為 `TEXT` (`HH:MM:SS[.ffffff]`) | `time` | 不含日期 |
| `datetime` | 日期時間 / 時間戳 | `timestamp` / `timestamptz` | `datetime` / `timestamp` | 常見為 ISO 8601 `TEXT` 或 Unix time `INTEGER` | `datetime2` / `datetimeoffset` | 若來源型別支援時區則會保留 |
| `uuid` | 128-bit 識別值 | `uuid` | 常見為 `binary(16)` 或 `char(36)` | 常見為 `TEXT` 或 `BLOB` | `uniqueidentifier` | MySQL 與 SQLite 無通用原生 UUID 型別 |
| `hex` | Binary payload | `bytea` | `binary` / `varbinary` / `blob` | `BLOB` | `binary` / `varbinary` | 文字格式採 hex |
| `b64` | Binary payload | `bytea` | `binary` / `varbinary` / `blob` | `BLOB` | `binary` / `varbinary` | 文字格式採 Base64 |

## 序列化方向

| Python 型別 | Typed Output |
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

| JS 值 | Typed Output |
| :--- | :--- |
| `null` | `null` |
| `boolean` | `true` / `false` |
| `number`（安全整數） | `42` |
| `number`（浮點數） | `3.14` |
| `string` | `"Alice"` |
| `bigint`（有號 i64 範圍內） | `42` |
| `Date` | `2026-03-08T14:30:00.000Z` |
| `Array` | `[1, 2]` |
| `plain object` | `{"name": "Alice"}` |
| `Uint8Array` | `hex(...)` 或 `b64(...)` |
| `toon.decimal('123.45')` | `123.45m` |
| `toon.uuid('...')` | `uuid(...)` |
| `toon.date('...')` | `2026-03-08` |
| `toon.time('14:30:00')` | `14:30:00` |
| `toon.datetime('2026-03-08T14:30:00+08:00')` | `2026-03-08T14:30:00+08:00` |

JS 的 `Date` 會透過 `Date.toISOString()` 序列化，因此輸出一律是 UTC，且會帶毫秒精度。

## 相關頁面

- **[格式偵測](./format-detection.md)** — 精確的自動偵測規則與 parser routing
- **[API Matrix](./api-matrix.md)** — batch、streaming、Arrow、transcode 能力矩陣
- **[JS codec 與 Int64](../guides/js-codecs-and-int64.md)** — 如何覆寫 JS 預設型別對應
