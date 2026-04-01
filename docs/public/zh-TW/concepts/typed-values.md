---
title: 具型別值 (Typed Values)
sidebar_position: 3
sidebar_label: 具型別值
description: T-TOON 和 T-JSON 共用的 12 種具型別值編碼完整參考。
---

# 具型別值 (Typed Values)

TTOON 定義了 12 種具型別的值編碼 — 稱為 **typed types** — 這些編碼在 T-TOON 和 T-JSON 語法之間共享。單個編碼實例稱為 **typed unit** (例如 `true`, `2026-03-01`, `uuid(...)`)。

## 術語定義

- **typed**：一種整體設計概念，表示值的文字編碼本身就攜帶型別語意。
- **typed types**：12 種內建值編碼的集合。
- **typed unit**：單一序列化後的值片段，也就是解析器直接面對的具體值文字。
- `hex` 與 `b64` 雖然都對應 binary payload，但在文字層仍各自算一種 typed type。

## 型別表

| 型別 | 語法 | 範例 |
| :--- | :--- | :--- |
| `null` | 關鍵字 | `null` |
| `bool` | 關鍵字 | `true`, `false` |
| `int` | 數字，包含可選正負號和 `_` 分隔符 | `42`, `-1_000`, `0` |
| `float` | 小數點或科學記號 | `3.14`, `1e-9`, `-0.5`, `inf`, `-inf`, `nan` |
| `decimal` | 帶有 `m` 後綴的數字 | `123.45m`, `0.00m`, `-99.9m` |
| `string` | 雙引號包圍 | `"Alice"`, `"hello\nworld"` |
| `date` | `YYYY-MM-DD` | `2026-03-08` |
| `time` | `HH:MM:SS[.fractional]` | `14:30:00`, `14:30:00.123456` |
| `datetime` | ISO 8601 | `2026-03-08T14:30:00`, `2026-03-08T14:30:00+08:00` |
| `uuid` | `uuid(...)` 包裝器 | `uuid(550e8400-e29b-41d4-a716-446655440000)` |
| `hex` | `hex(...)` 包裝器 | `hex(4A42)` |
| `b64` | `b64(...)` 包裝器 | `b64(SkI=)` |

## 型別詳細說明

### null

`null` — 單一個關鍵字。表示缺少值。

### bool

`true` 或是 `false` — 僅限小寫。沒有其他表示 true/false 的寫法。

### int

整數值，帶有可選的正負號前綴和 `_` 數字分隔符 (為了提升可讀性)。

```text
42
-1_000_000
0
```

**JS 精度注意事項**：JS 原生的 `number` 只能安全地表示 `-(2^53 - 1)` 到 `2^53 - 1` 範圍內的整數。預設情況下，超出此範圍的值將引發錯誤。使用 `intBigInt()` 編解碼器來接收 `BigInt`，或使用 `intNumber({ overflow: 'lossy' })` 來顯式允許精度的損失。請參閱 [JS 編解碼器與 Int64](../guides/js-codecs-and-int64.md)。

### float

浮點數值。必須包含小數點或使用科學記號。

```text
3.14
1e-9
-0.5
inf
-inf
nan
```

### decimal

帶有小寫 `m` 後綴的精確十進位值。不使用科學記號 — 保留完整精度。

```text
123.45m
0.00m
-99.9m
```

`m` 後綴區分了十進位和浮點數：`123.45` 是浮點數，`123.45m` 是十進位。

### string

始終使用雙引號。不允許 bare token（`compat` 模式下未知的 bare token 會被接受為字串）。

```text
"Alice"
"hello world"
""
```

**T-TOON 跳脫規則**：僅允許 `\\`, `\"`, `\n`, `\r`, `\t`。其他跳脫 (例如 `\uXXXX`) 會被拒絕。

**T-JSON 跳脫規則**：完整的 JSON 跳脫集，包含 `\uXXXX`, `\b`, `\f`。

### date

`YYYY-MM-DD` 格式。在 T-TOON 和 T-JSON 中都是純字詞 (無引號)。

```text
2026-03-08
```

### time

`HH:MM:SS` 加上可選達微秒精度的秒小數部分。

```text
14:30:00
14:30:00.123456
```

### datetime

帶有可選時區的 ISO 8601 格式。

```text
2026-03-08T14:30:00
2026-03-08T14:30:00+08:00
2026-03-08T14:30:00Z
```

### uuid

包裝在 `uuid(...)` 中，以防止被誤認為是普通字串。

```text
uuid(550e8400-e29b-41d4-a716-446655440000)
```

UUID 必須是 36 個字元長的標準 8-4-4-4-12 格式。
僅接受 lowercase (小寫) 十六進位；大寫十六進位將被拒絕。

### hex / b64 (binary payloads)

二進位數據使用十六進位 (hexadecimal) 或 base64 編碼：

```text
hex(48656C6C6F)
b64(SGVsbG8=)
```

`hex` 和 `b64` 都表示二進位資料，但在 TTOON 的術語中仍分別算作兩種 typed type。它們共享相同的執行期對應 (`bytes`、`Uint8Array`、Arrow `Binary`)，差異只在文字編碼形式。

## 跨語言對應 (Cross-Language Mapping)

| 具型別型別 | Python `loads()` | JS `parse()` 預設 | Arrow Schema |
| :--- | :--- | :--- | :--- |
| `null` | `None` | `null` | 允許為 null 的欄位 (Nullable column) |
| `bool` | `bool` | `boolean` | `Boolean` |
| `int` | `int` | `number` (溢位則出錯) | `Int64` |
| `float` | `float` | `number` | `Float64` |
| `decimal` | `decimal.Decimal` | `string` (移除 `m` 後綴) | `Decimal128/256` |
| `string` | `str` | `string` | `Utf8` |
| `date` | `datetime.date` | `string` | `Date32` |
| `time` | `datetime.time` | `string` | `Time64(Microsecond)` |
| `datetime` | `datetime.datetime` | `string` | `Timestamp(Microsecond[, tz])` |
| `uuid` | `uuid.UUID` | `string` | `FixedSizeBinary(16)` + UUID 元資料 |
| `hex`/`b64` | `bytes` | `Uint8Array` | `Binary` |

**為什麼 JS 會為某些型別回傳字串**：JS 沒有原生的 `Decimal`, `Date` (只有時間), 或是 `UUID` 型別。回傳字串可以避免強迫引入第三方依賴項。編解碼器可以複寫這個對應行為 — 請參閱 [JS 編解碼器與 Int64](../guides/js-codecs-and-int64.md)。

如需包含 Arrow 資訊在內的完整跨語言型別矩陣，請參閱 [型別對應 (Type Mapping)](../reference/type-mapping.md)。
