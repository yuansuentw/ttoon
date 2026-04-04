---
title: 直接轉碼
sidebar_position: 4
sidebar_label: 轉碼
description: 在不具現化特定語言原生物件的情況下，直接在 T-JSON 和 T-TOON 之間轉換。
---

# 直接轉碼

TTOON 提供直接轉碼的 API，只需通過 Rust 的內部表示 (IR) 即可在 T-JSON 和 T-TOON 格式之間進行轉換 — 不會具現化任何 Python 物件、JS 值或 Arrow 表格。這種方式以極低的開銷保留了所有的 typed 語意 (decimal, uuid, date, binary 等等)。

## 它是如何運作的

```text
T-JSON 文字 ──parse──→ Rust IR ──serialize──→ T-TOON 文字
T-TOON 文字 ──parse──→ Rust IR ──serialize──→ T-JSON 文字
```

文字會被解析為內部表示 (IR)，然後立即序列化為目標格式。過程中永遠不會建立特定語言的原生物件，使其成為轉換格式最有效率的方法。

## Python

```python
import ttoon

# T-JSON → T-TOON
ttoon_text = ttoon.tjson_to_ttoon('{"name": "Alice", "scores": [95, 87]}')
# name: "Alice"
# scores:
#   [2]: 95, 87

# T-TOON → T-JSON
tjson_text = ttoon.ttoon_to_tjson('name: "Alice"\nage: 30')
# {"name": "Alice", "age": 30}
```

**選項：**

| 函數 | 參數 |
| :--- | :--- |
| `tjson_to_ttoon(text)` | `delimiter`, `indent_size`, `binary_format` |
| `ttoon_to_tjson(text)` | `mode`, `binary_format` |

`tjson_to_ttoon()` 使用專用的嚴格 (strict) T-JSON 解析器 — 它不接受 `mode` 參數。

## JavaScript / TypeScript

```ts
import { tjsonToTtoon, ttoonToTjson } from '@ttoon/shared';

// T-JSON → T-TOON
const ttoonText = tjsonToTtoon('{"name": "Alice", "age": 30}');

// T-TOON → T-JSON
const tjsonText = ttoonToTjson('name: "Alice"\nage: 30');

// 使用選項
const result = tjsonToTtoon(text, { delimiter: '\t', binaryFormat: 'b64' });
const result2 = ttoonToTjson(text, { mode: 'strict' });
```

**選項：**

| 函數 | 選項介面 (Interface) |
| :--- | :--- |
| `tjsonToTtoon(text, opts?)` | `TjsonToTtoonOptions` — 繼承自 `SerializeOptions` |
| `ttoonToTjson(text, opts?)` | `TtoonToTjsonOptions` — 繼承自 `TjsonSerializeOptions` + `mode` |

## Rust

```rust
use ttoon_core::{tjson_to_ttoon, ttoon_to_tjson, BinaryFormat, Delimiter, ParseMode, TjsonOptions, TtoonOptions};

// T-JSON → T-TOON (總是進行嚴格解析)
let ttoon = tjson_to_ttoon(r#"{"key": 42}"#, None)?;

// T-TOON → T-JSON (解析模式可設定)
let tjson = ttoon_to_tjson("key: 42", ParseMode::Compat, None)?;

// 附帶編碼選項
let opts = TtoonOptions {
    binary_format: BinaryFormat::Hex,
    indent_size: 4,
    delimiter: Delimiter::Comma,
};
let json_opts = TjsonOptions {
    binary_format: BinaryFormat::B64,
};
let ttoon_with_opts = tjson_to_ttoon(r#"{"key": 42}"#, Some(&opts))?;
let tjson_with_opts = ttoon_to_tjson("key: 42", ParseMode::Compat, Some(&json_opts))?;
```

## 錯誤處理

轉碼錯誤會始終包含操作名稱。階段 (phase) 的報告則因語言而異：

- Python 和 Rust：`phase` 準確反映了是發生在解析 (parse) 還是序列化 (serialize) 階段。
- JavaScript：目前直接轉碼的 `phase` 始終報告為 `'parse'`，因為 WASM 橋接將這整個操作暴露為單一呼叫。

```python
from ttoon import TranscodeError

try:
    ttoon.tjson_to_ttoon("invalid{json")
except TranscodeError as e:
    print(e)  # operation: tjson_to_ttoon, phase: parse, ...
```

```ts
import { TranscodeError } from '@ttoon/shared';

try {
  tjsonToTtoon('invalid{json');
} catch (e) {
  if (e instanceof TranscodeError) {
    console.log(e.operation); // 'tjson_to_ttoon'
    console.log(e.phase);     // 在 JS 中目前總是 'parse'
    console.log(e.sourceKind); // 底層來源錯誤的種類
  }
}
```

## 關鍵行為

- **保留所有 typed value**：`decimal(m)`, `uuid(...)`, `date`, `time`, `datetime`, `hex(...)`, `b64(...)` 全部都能完好無損地在轉換中保留下來。
- **T-JSON 解析總是嚴格的 (strict)**：`tjson_to_ttoon()` 不接受 `mode` 參數 — 根據定義，T-JSON 便是嚴格的。
- **T-TOON 解析使用 `mode`**：`ttoon_to_tjson()` 預設為 `compat` 模式，但是可以明確指定為 `strict`。
- **沒有物件具現化**：在轉換期間，不會建立任何 Python 的 `dict`、JS `object` 或 Arrow 表格。
- **JS 階段 (phase) 的注意事項**：JS 的 `TranscodeError.phase` 目前是一個粗略的包裝器欄位，並不是可靠的 parsing/serializing 判別器。
