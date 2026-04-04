---
title: T-TOON vs T-JSON
sidebar_position: 2
sidebar_label: T-TOON vs T-JSON
description: T-TOON (基於縮排) 和 T-JSON (基於括號) 語法的詳細比較。
---

import CodeBlock from '@theme/CodeBlock';

# T-TOON vs T-JSON

TTOON 為不同的使用情境提供了兩種語法。兩者共用相同的 typed value 層 — 差異純粹在於結構。

T-TOON 是建立在 [TOON](https://toonformat.dev/) 之上的 typed 擴充，而原始 TOON 來自 [toon-format 專案](https://github.com/toon-format/toon)。換言之，這份比較中的 T-TOON 一側直接承襲了 TOON 以縮排表達物件、以原生 tabular header 表達統一陣列的做法；T-JSON 則是 TTOON 額外提供的括號式配套語法。

## 比較範例

### 簡單物件

<div className="cmp-grid">
  <div className="cmp-card">
    <div className="cmp-card__label">T-TOON</div>
    <CodeBlock language="text">{`name: "Alice"
age: 30
active: true`}</CodeBlock>
  </div>
  <div className="cmp-card">
    <div className="cmp-card__label">T-JSON</div>
    <CodeBlock language="json">{`{
  "name": "Alice",
  "age": 30,
  "active": true
}`}</CodeBlock>
  </div>
</div>

### 巢狀物件

<div className="cmp-grid">
  <div className="cmp-card">
    <div className="cmp-card__label">T-TOON</div>
    <CodeBlock language="text">{`user:
  name: "Alice"
  address:
    city: "Taipei"
    zip: "100"`}</CodeBlock>
  </div>
  <div className="cmp-card">
    <div className="cmp-card__label">T-JSON</div>
    <CodeBlock language="json">{`{
  "user": {
    "name": "Alice",
    "address": {
      "city": "Taipei",
      "zip": "100"
    }
  }
}`}</CodeBlock>
  </div>
</div>

### 陣列

<div className="cmp-grid">
  <div className="cmp-card">
    <div className="cmp-card__label">T-TOON</div>
    <CodeBlock language="text">{`scores[3]: 95, 87, 92`}</CodeBlock>
  </div>
  <div className="cmp-card">
    <div className="cmp-card__label">T-JSON</div>
    <CodeBlock language="json">{`[
  95,
  87,
  92
]`}</CodeBlock>
  </div>
</div>

### 表格資料 (統一物件的列表)

<div className="cmp-grid">
  <div className="cmp-card">
    <div className="cmp-card__label">T-TOON</div>
    <CodeBlock language="text">{`[3]{name,score,grade}:
"Alice", 95, "A"
"Bob", 87, "B"
"Carol", 92, "A"`}</CodeBlock>
  </div>
  <div className="cmp-card">
    <div className="cmp-card__label">T-JSON</div>
    <CodeBlock language="json">{`[
  {
    "name": "Alice",
    "score": 95,
    "grade": "A"
  },
  {
    "name": "Bob",
    "score": 87,
    "grade": "B"
  },
  {
    "name": "Carol",
    "score": 92,
    "grade": "A"
  }
]`}</CodeBlock>
  </div>
</div>

### typed value

兩種語法使用完全相同的 typed value 編碼：

```text
amount: 123.45m
id: uuid(550e8400-e29b-41d4-a716-446655440000)
created: 2026-03-08T14:30:00+08:00
blob: hex(4A42)
```

## 結構差異

| 比較點 | T-TOON | T-JSON |
| :--- | :--- | :--- |
| 結構 | 基於縮排 | 基於括號 (`{}` / `[]`) |
| 物件鍵 (Object keys) | 純標識符後接 `: ` | 引號包圍的字串 (`"key"`) |
| 表格格式 | 原生的 `[N]{fields}:` 標頭 | 物件的陣列 |
| 可讀性 | 針對人類進行最佳化 | 較接近 JSON |
| escape 規則 | 僅 5 種 escape (`\\`, `\"`, `\n`, `\r`, `\t`) | 完整的 JSON escape 規則集 |
| 串流標頭 | `[*]{fields}:` (無界限) | 最外層物件陣列 |
| 巢狀 | 使用縮排深度 | 使用括號深度 |

## 該在何時選擇哪一種

### 何時使用 T-TOON

- 人類可讀性是首要考量 (設定檔、日誌、除錯)
- 資料是表格格式 — `[N]{fields}:` 格式比重複的 JSON 物件緊湊得多
- 您需要對結構化資料進行輕鬆的 `diff` 和 `grep`
- 處理串流表格資料 (`[*]{fields}:` 標頭)

### 何時使用 T-JSON

- 下游系統期望類似 JSON 的結構
- 您需要完整的 JSON escape 支援 (`\uXXXX`, `\b`, `\f`)
- 偏好基於括號的巢狀結構，而非縮排
- 與現有的 JSON 工具 (編輯器、驗證器、日誌處理器) 互操作

### 兩者皆可的情境

- 跨語言交換 — 所有 SDK 都會在解析時自動偵測格式
- Arrow / Polars 整合 — 兩種格式都支援 `readArrow()`

## 解析行為

解析器會根據第一個有意義的內容自動偵測格式：

1. 第一個非空白字元是 `{` → T-JSON
2. 第一行是 `[N]:` 或 `[N]{fields}:` → T-TOON 表格
3. 第一個非空白字元是 `[` 但它不符合 T-TOON 表格標頭 → T-JSON
4. 否則 → `typed_unit`

`detect_format()` 沒有獨立的 "T-TOON 縮排" 結果。基於縮排的物件和純量 typed value 最初都會被視為 `typed_unit`，然後 T-TOON 解析器會在解析路徑上區分它們。一旦選定了一條格式路線，解析器就會執行到底。不會有靜默的退回 (fallback) 機制。

## 直接轉換

您可以在不具現化特定語言原生物件的情況下轉換格式：

```python
import ttoon

# T-JSON → T-TOON (僅通過 Rust IR)
ttoon_text = ttoon.tjson_to_ttoon('{"name": "Alice", "age": 30}')

# T-TOON → T-JSON (僅通過 Rust IR)
tjson_text = ttoon.ttoon_to_tjson('name: "Alice"\nage: 30')
```

```ts
import { tjsonToTtoon, ttoonToTjson } from '@ttoon/shared';

const ttoonText = tjsonToTtoon(`{
  "name": "Alice",
  "age": 30
}`);

const tjsonText = ttoonToTjson(`name: "Alice"
age: 30`);
```

詳情請參閱[轉碼指南](../guides/transcode.md)。

## 以行分隔的資料列

沒有表格標頭且單純以逗號分隔的純文字行是**無效的 T-TOON**：

```text
1, 2, 3
4, 5, 6
```

請改用帶有標頭的 `T-TOON` 表格，或使用 `T-JSON` 陣列。
