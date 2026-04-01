---
title: 格式總覽 (Format Overview)
sidebar_position: 3
sidebar_label: 格式總覽
description: T-TOON 和 T-JSON 語法總覽、具型別值系統以及跳脫規則。
---

# 格式總覽 (Format Overview)

TTOON 提供兩種語法 — T-TOON 和 T-JSON — 它們共用相同的具型別值層 (typed value layer)，但在結構語法上有所不同。本頁面涵蓋了兩者的基本特徵。

## 術語速覽

- **typed**：指「值的文字表示本身帶有型別語意」這個整體設計概念。
- **typed types**：指內建的 12 種值編碼類別。
- **typed unit**：指單一實際出現的值片段，例如 `123.45m`、`2026-03-08`、`uuid(...)`。
- `hex` 與 `b64` 在文件中分開列出，因為它們是兩種不同的 typed type；只是兩者都用來表示 binary payload。

## T-TOON 語法

T-TOON 使用基於縮排的結構，沒有多餘的括號。它專為人類閱讀和手動編輯而設計。

### 物件 (Object)

```text
name: "Alice"
age: 30
tags[2]: "admin", "ops"
```

### 巢狀物件 (Nested Object)

```text
user:
  name: "Alice"
  address:
    city: "Taipei"
```

### 表格 (統一物件的列表)

```text
[2]{name,score}:
"Alice", 95
"Bob", 87
```

標頭 `[N]{fields}:` 宣告了列數和欄位名稱。分隔符號可以是逗號 (預設)、定位字元 (tab) 或垂直線 (pipe)。

## T-JSON 語法

T-JSON 使用類似 JSON 的 `{}` / `[]` 括號。物件的鍵 (keys) 必須是引號包圍的字串 (遵循 JSON 規則)。值層面使用具型別語法，而不是純 JSON 值。

### 物件 (Object)

```text
{"name": "Alice", "amount": 123.45m, "id": uuid(550e8400-e29b-41d4-a716-446655440000)}
```

### 陣列 (Array)

```text
[1, 2, 3]
```

### 巢狀 (Nested)

```text
{"user": {"name": "Alice", "scores": [95, 87]}}
```

## 具型別值系統 (Typed Value System)

這兩種語法共用相同的 12 種具型別值編碼，也就是同一組 `typed types`：

| 型別 | 範例 | 備註 |
| :--- | :--- | :--- |
| `null` | `null` | Null 值 |
| `bool` | `true` / `false` | 小寫關鍵字 |
| `int` | `42` / `-1_000` | 允許正負號前綴和 `_` 分隔符號 |
| `float` | `3.14` / `1e-9` / `inf` / `nan` | 浮點數，支援科學記號和特殊值 |
| `decimal` | `123.45m` | 小寫 `m` 後綴；不使用科學記號；保留完整精度 |
| `string` | `"Alice"` | 始終使用雙引號包圍 |
| `date` | `2026-03-08` | `YYYY-MM-DD` |
| `time` | `14:30:00.123456` | 精度高達微秒 |
| `datetime` | `2026-03-08T14:30:00+08:00` | ISO 8601 可選包含時區 |
| `uuid` | `uuid(550e8400-e29b-41d4-a716-446655440000)` | 包裝器，防止被誤認為是字串 |
| `hex(...)` | `hex(4A42)` | 十六進位二進位編碼 |
| `b64(...)` | `b64(SkI=)` | Base64 二進位編碼 |

### 為何是這 12 種 typed types

TTOON 的 `typed types` 不是照抄某一個資料庫或語言的型別系統，而是取主流 RDBMS、Arrow，以及 Python / JS / Rust 三個 SDK 之間最常用、最穩定的交集。

設計目標是提供一組：

- 足以覆蓋跨系統資料交換常見需求的核心型別
- 能在 object path 與 Arrow path 都保持穩定語意的型別
- 不必綁死單一資料庫私有型別的可攜層

因此，像 `jsonb`、`enum`、`interval`、`array`、地理型別、`money` 這類偏資料庫專屬或語意差異過大的型別，暫時不作為內建 typed type；它們應由上層 schema、欄位慣例或自訂 codec 處理。

### 關鍵規則

- **字串始終需要引號**：所有字串值都使用雙引號 `"..."` 來消除純標記字詞 (bare token) 的歧義。
- **Decimal 使用 `m` 後綴**：`123.45m` — 這區分了精確的十進位與浮點數 `123.45`。
- **UUID 和二進位使用包裝器**：`uuid(...)`、`hex(...)`、`b64(...)` — 防止被誤認為是字串。

## 跳脫規則 (Escape Rules)

### T-TOON

T-TOON 僅允許 5 種跳脫字元：`\\`、`\"`、`\n`、`\r`、`\t`。任何其他跳脫 (例如 `\uXXXX`) 都會被拒絕。

### T-JSON

T-JSON 遵循完整的 JSON 跳脫規則集，包括 `\uXXXX` Unicode 跳脫、`\b`、`\f` 等等。

## 格式偵測 (Format Detection)

TTOON 會自動偵測輸入格式：

- 第一個非空白字元是 `{` → **T-JSON**
- 第一行匹配表格標頭 `[N]{fields}:` 或 `[N]:` → **T-TOON** 表格
- 第一個非空白字元是 `[` 但它不匹配表格標頭 → **T-JSON**
- 其他情況 → 來自 `detect_format()` 的 `typed_unit`；接著 T-TOON 解析器會將基於縮排的結構與單一的具型別值區分開來

確定格式後，解析器不會退回到另一種格式。

## 該使用哪一種？

| 情境 | 建議 |
| :--- | :--- |
| 人類可讀的設定檔、日誌或差異比對 | T-TOON |
| 大型表格數據集 | T-TOON 表格 |
| 與基於 JSON 的系統互動 | T-JSON |
| 需要基於括號的結構 | T-JSON |
| 跨語言物件交換 | 任一皆可 (在解析時自動偵測) |

## 下一步

- **[為何選擇 TTOON？ (Why TTOON?)](../concepts/why-ttoon.md)** — 更深層的動機與定位
- **[T-TOON vs T-JSON](../concepts/ttoon-vs-tjson.md)** — 詳細比較
- **[具型別值 (Typed Values)](../concepts/typed-values.md)** — 完整的型別參考與跨語言行為
- **[型別對應 (Type Mapping)](../reference/type-mapping.md)** — 各語言、Arrow 與主流 RDBMS 的型別映射
