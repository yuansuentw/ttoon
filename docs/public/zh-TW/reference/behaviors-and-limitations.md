---
title: 行為與限制 (Behaviors & Limitations)
sidebar_position: 6
sidebar_label: 行為與限制
description: TTOON 之邊界情況、約束與設計層級的行為。
---

# 行為與限制 (Behaviors & Limitations)

## 空輸入 (Empty Input)

空字串、僅含空白或僅含換行的輸入，目前會被解析為空物件 `{}`，而非 `null`。

## 字串必須加上引號

T-TOON 和 T-JSON 中的所有字串值都必須使用雙引號。這消除了 bare token 的歧義 — 不存在「無引號字串」這種型別。

T-JSON 的物件鍵 (key) 也必須是加上引號的字串，遵循 JSON 規則。

## 行分隔資料列

沒有表格標頭的純逗號分隔行**不是**有效的 T-TOON 格式：

```text
1, 2, 3
4, 5, 6
```

請改用帶有 `[N]{fields}:` 標頭的 T-TOON 表格，或 T-JSON 陣列。

## Arrow 輸入要求

所有語言的 `read_arrow()` 皆有以下強制要求：

- 根部必須為列表
- 每個元素必須為物件
- 各欄位型別在所有列中必須一致（同一欄必須為相同純量型別）
- 結構性欄位（列表/物件）無法以 Arrow 表示

## JS int64 精度

JS 原生 `number` 只能安全表示 `-(2^53 - 1)` 到 `2^53 - 1` 範圍內的整數。

- **預設行為**：超出安全範圍時拋出錯誤 — 不會有靜默的精度流失
- **`intBigInt()`**：將所有整數對應為 `BigInt`
- **`intNumber({ overflow: 'lossy' })`**：明確接受精度流失

## JS 預設以 String 回傳

JS 對 `decimal`、`date`、`time`、`datetime` 和 `uuid` 預設回傳 `string`。這是為了避免將使用者綁定到特定的第三方依賴。可透過 `use()` 註冊編解碼器來改變此行為。

## 編解碼器作用範圍 (Codec Scope)

編解碼器是語言特定的物件路徑轉接層：

- **JavaScript**：影響 `parse()`、`stringify()`、`toTjson()` 及物件路徑的串流讀寫器
- **Python**：僅影響物件路徑的串流讀寫器

以下**不受**影響：

- T-TOON / T-JSON 語法
- Rust 核心行為
- Arrow schema 推論規則
- Arrow 串流 API
- 直接轉碼 API（`tjson_to_ttoon()` / `ttoon_to_tjson()`、`tjsonToTtoon()` / `ttoonToTjson()`）

## Arrow Datetime 時區一致性

JS 的 Arrow 橋接器不允許在同一欄位中混用帶時區和不帶時區的 datetime。混用會導致 schema 推論錯誤。

## 格式偵測即承諾 (Format Detection is Commitment)

一旦 `detect_format()` 判定輸入為 T-TOON 或 T-JSON，解析器便會提交至該格式。解析錯誤時不會靜默退回至另一格式。

## 解析即驗證 (Parse is Validation)

TTOON 沒有獨立的 `validate()` API。型別有效性在解析過程中一併檢查：

- UUID 格式正確性（36 字元，8-4-4-4-12）
- Decimal `m` 後綴是否存在
- T-TOON 字串跳脫合規性（僅允許 5 種）
- T-JSON 物件鍵必須為字串
- Arrow 路徑的欄位型別一致性

## T-TOON 與 T-JSON 的跳脫差異 (Escape Asymmetry)

T-TOON 僅支援 5 種跳脫序列：`\\`、`\"`、`\n`、`\r`、`\t`。T-JSON 支援完整的 JSON 跳脫集，包含 `\uXXXX`、`\b`、`\f`。在 T-TOON 中使用 T-JSON 的跳脫序列會導致解析錯誤。

## 串流標頭慣例 (Streaming Header Convention)

- **T-TOON 批次**：`[N]{fields}:` — `N` 為確切列數
- **T-TOON 串流**：`[*]{fields}:` — `*` 表示無限制列數
- **T-JSON 串流**：頂層物件陣列，schema 已知的值僅限純量與 null
