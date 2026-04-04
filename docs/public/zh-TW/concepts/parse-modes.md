---
title: 解析模式
sidebar_position: 4
sidebar_label: 解析模式
description: 了解 TTOON 中的 compat 和 strict 解析模式。
---

# 解析模式

TTOON 支援兩種解析模式，這控制了在 T-TOON 解析期間要如何處理未知的無引號標記 (bare tokens)。

## `compat` 模式

未知的純標記字詞將退化 (fallback) 為字串。這與 TOON v3.0 的行為相容，在 v3.0 中，無引號的字串是有效的。

```text
key: hello
```

在 `compat` 模式中，`hello` 將被解析為字串 `"hello"`。

## `strict` 模式

未知的純標記字詞會導致立即的錯誤。這適合用於由機器產生的資料，其中每個值都應該有明確的型別。

```text
key: hello    → 錯誤: 未知的純標記字詞 "hello"
key: "hello"  → 成功: 字串 "hello"
key: 42       → 成功: int 42
key: true     → 成功: bool true
```

## 哪些格式會受到模式影響

| 格式 | 受到 `mode` 影響嗎？ |
| :--- | :--- |
| T-TOON 縮排 (indentation) | 是 |
| T-TOON 表格 (tabular) | 是 |
| T-JSON 批次 / 直接轉碼 | **否** — 結構解析一律嚴格 |
| 帶 schema 的 T-JSON 串流 | **是** — schema 外欄位的處理會跟隨 `mode` |
| 具型別單位 (Typed unit) | 是 |

對批次解析與直接轉碼來說，T-JSON 無論 `mode` 設定為何，結構解析都一律是 strict，因為 T-JSON 遵循 JSON 結構規則，所有字串值都必須加上引號。不過在帶 schema 的 T-JSON 串流 reader 中，`mode` 仍會影響 schema 不符時的處理方式：`compat` 會略過未知欄位，`strict` 會直接報錯；JSON 值語法本身則在兩種模式下都維持嚴格。

## 依語言的預設值

- Python `loads()` 和 `ttoon_to_tjson()` 預設為 `compat`
- JS `parse()` 和 `ttoonToTjson()` 預設為 `compat`
- Rust 的便利 API (像是 `from_ttoon()`) 預設為 `compat`
- Rust 的 `ParseMode::default()` 是 `Strict`

## 使用方式

### Python

```python
import ttoon

# compat (預設)
data = ttoon.loads('key: hello')         # {"key": "hello"}

# strict
data = ttoon.loads('key: "hello"', mode="strict")  # 成功
data = ttoon.loads('key: hello', mode="strict")     # 錯誤
```

### JavaScript / TypeScript

```ts
import { parse } from '@ttoon/shared';

parse('key: hello');                         // { key: "hello" }
parse('key: hello', { mode: 'strict' });     // 錯誤
parse('key: "hello"', { mode: 'strict' });   // 成功
```

### Rust

```rust
use ttoon_core::{from_ttoon, from_ttoon_with_mode, ParseMode};

let node = from_ttoon("key: hello")?;                             // compat 便利 API
let node = from_ttoon_with_mode("key: hello", ParseMode::Strict); // 錯誤
let mode = ParseMode::default();                                  // Strict
```

## 建議

| 情境 | 模式 |
| :--- | :--- |
| 人類編寫的設定檔/資料 | `compat` |
| 機器產生的輸出 | `strict` |
| 跨語言交換 | `strict` (確保有顯式的型別) |
| 舊版 TOON v3.0 資料 | `compat` |
| 帶有 Schema 的串流 | 兩者皆可 (也會影響 T-JSON 串流的未知欄位處理) |

## 與轉碼的互動

- `tjson_to_ttoon()` — T-JSON 解析始終是嚴格的；沒有 `mode` 參數
- Python/JS `ttoon_to_tjson()` / `ttoonToTjson()` — 接受 `mode`，預設為 `compat`
- Rust `ttoon_to_tjson()` — 必須提供 `mode`；Rust 方面沒有預設參數
