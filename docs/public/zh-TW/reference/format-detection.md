---
title: 格式偵測
sidebar_position: 9
sidebar_label: 格式偵測
description: TTOON 如何在 T-TOON, T-JSON 和具型別單位文字間自動偵測輸入的格式。
---

# 格式偵測

所有的 TTOON 解析 API (`loads()`, `parse()`, `from_ttoon()`, `read_arrow()`, `readArrow()`) 都會由自動的格式偵測作為起始。而 `detect_format()` / `detectFormat()` 這個函數直接將這套偵測邏輯開放給大眾使用。

## 回傳值

| 結果 | 意義 |
| :--- | :--- |
| `"tjson"` / `'tjson'` | T-JSON (基於括號) |
| `"ttoon"` / `'ttoon'` | T-TOON (基於縮排或者是表格) |
| `"typed_unit"` / `'typed_unit'` | 一個單一的 typed value (例如 `42`, `true`, `uuid(...)`) |

## 偵測規則

偵測器會檢驗首行第一個具意義的 (非空白的) 字元：

1. **第一個非空白字元是 `{`** → `tjson`
2. **第一個非空白字元是 `[`** 且該行符合了 TOON 表格的標頭與格式模式 → `ttoon`
3. **第一個非空白字元是 `[`** 但該行不符合 TOON 表格的標頭與格式模式 → `tjson`
4. **除此之外的任何其他東西** → `typed_unit`

解析器仍有可能在偵測階段之後將屬性為 `typed_unit` 的輸入，發送轉導至 T-TOON 解析器中，但也就是說 `detect_format()` 它本身並不會真的有去探查其是否包含了類似 `key: value` 內容。

## 實際應用方式

### Python

```python
import ttoon

ttoon.detect_format('{"name": "Alice"}')                    # "tjson"
ttoon.detect_format('[2]{name,score}:\n"Alice", 95\n"Bob", 87')  # "ttoon"
ttoon.detect_format('name: "Alice"\nage: 30')               # "typed_unit"
ttoon.detect_format('42')                                    # "typed_unit"
ttoon.detect_format('true')                                  # "typed_unit"
```

### JavaScript / TypeScript

```ts
import { detectFormat } from '@ttoon/shared';

detectFormat('{"key": 42}');       // 'tjson'
detectFormat('key: 42');           // 'typed_unit'
detectFormat('true');              // 'typed_unit'
```

### Rust

```rust
use ttoon_core::detect_format;
use ttoon_core::format_detect::Format;

let fmt = detect_format("{\"key\": 42}");
assert_eq!(fmt, Format::Tjson);

let fmt = detect_format("key: 42");
assert_eq!(fmt, Format::TypedUnit);
```

## 關鍵的行為特點

### 沒有替補與退路機制

一旦判斷出格式，解析器便會全程採用該格式。若解析失敗，錯誤會以偵測到的格式回報 — 解析器**不會**靜默地改用其他格式重試。

### T-JSON 格式的偵測

T-JSON 格式偵測觸發的時機為：在遇到 `{` ，或者是遇到 `[` 且又遇到此行格式與 TOON 標題模式不符合時。這意味著：

- `[1, 2, 3]` → T-JSON 陣列
- `[2]{a,b}:` → T-TOON 表格形式 (這儘管是以 `[` 起頭但它並非是 T-JSON) 

偵測器將藉由是否發現有遵循並遵守著 `[` 之接續規則來辨別出這段字串到底是屬於 T-JSON 陣列還是 T-TOON 表格的標頭。

### 串流標頭

`[*]{fields}:` 的串流標頭將會被偵測並導向為 T-TOON，而非 T-JSON。而 `*` 的這個標記特徵剛好就是串流與固定行數的批次版表格 (`[N]{fields}:`) 兩者之間的不同分別之處。

### 空輸入處理

空字串或者是只具有空白的輸入一樣會由偵測器處理。在這種情況下 `detect_format()` 將回傳出 `typed_unit`；而後接下來的各項 API 操作則將這些空白等同為視作為是一個空的物件 `{}` 來處置。
