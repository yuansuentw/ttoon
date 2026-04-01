---
title: 介紹 (Introduction)
sidebar_position: 1
sidebar_label: 介紹
description: TTOON — 適用於 Python、JavaScript 和 Rust 的具型別純文字數據交換格式。
---

# TTOON

TTOON 是一種為現代數據工作流程設計的**具型別純文字 (typed plain text)** 數據交換格式。它在一個專案中提供了兩種相輔相成的語法：

- **T-TOON** — 一種簡潔的、基於縮排的結構，並具有適用於資料集的原生表格佈局。
- **T-JSON** — 一種類似 JSON 的結構，在葉節點層級保留了具型別值的語法。

TTOON 是一個**獨立專案**。其中 `T-TOON` 是基於 TOON v3.0 延伸的縮排式語法，`T-JSON` 則是同一套 typed value system 的 JSON-like 結構語法；兩者是同一專案下的互補表示法，不是彼此無關的兩個格式。

## 為何選擇 TTOON？

大多數序列化格式會迫使您在可讀性或機器精度之間做出權衡。TTOON 拒絕妥協：

- **型別保真度 (Type fidelity)** — 跨語言邊界保留 `decimal`、`date`、`time`、`datetime`、`uuid` 和 `binary`，而不必將所有內容降級為字串。
- **人類可讀 (Human readable)** — 純文字輸出，易於閱讀、比較差異和進行視覺化除錯。
- **高效能 (High performance)** — 提供一流的 Apache Arrow 與 Polars 整合，並為表格數據提供零拷貝 (zero-copy) 路徑。
- **跨語言 (Cross-language)** — 透過共用的 Rust 核心引擎，在 Python、JavaScript/TypeScript 和 Rust 之間實現完全一致的行為。
- **輕量級執行期 (Lightweight runtime)** — 不需要完整的 Node.js；可在 Vercel 函數、Cloudflare Workers 和 Supabase Edge functions 中運行。

## 官方 SDK

| 語言 | 套件 | 架構 |
| :--- | :--- | :--- |
| Python | `ttoon` | 透過 PyO3 橋接的 Rust 核心 |
| JavaScript / TypeScript | `@ttoon/shared` | 透過 WASM 橋接的 Rust 核心 |
| JavaScript / Node.js | `@ttoon/node` | 重新匯出 `@ttoon/shared` |
| JavaScript / Web | `@ttoon/web` | 重新匯出 `@ttoon/shared` |
| Rust | `ttoon-core` | 核心引擎 (標準實作) |

所有三種語言的 SDK 都共用相同的 Rust 核心，確保了完全一致的解析和序列化行為。API 表面在所有語言中完全對齊 (18/18) — 涵蓋批次處理、串流和轉碼操作。

## 快速範例

### Python

```python
import ttoon

text = ttoon.dumps({"name": "Alice", "amount": 123.45})
data = ttoon.loads(text)
```

### JavaScript / TypeScript

```ts
import { parse, stringify } from '@ttoon/shared';

const text = stringify({ name: 'Alice', amount: 123.45 });
const data = parse(text);
```

### Rust

```rust
use ttoon_core::{from_ttoon, to_ttoon};

let node = from_ttoon("name: \"Alice\"\nage: 30")?;
let text = to_ttoon(&node, None)?;
```

## 核心功能

| 功能 | 描述 |
| :--- | :--- |
| **批次解析 / 序列化 (Batch parse / serialize)** | 雙向支援 `T-TOON` 和 `T-JSON`，適用於物件和 Arrow 表格 |
| **串流 I/O (Streaming I/O)** | 支援兩種格式的逐行讀取器與寫入器，提供物件和 Arrow 變體 |
| **直接轉碼 (Direct transcode)** | `T-JSON → T-TOON` 和 `T-TOON → T-JSON` 轉換，無需具現化為特定語言的原生物件 |
| **格式偵測 (Format detection)** | 基本輸入文字自動偵測 `tjson`、`ttoon` 或 `typed_unit` |
| **Schema 系統 (Schema system)** | 用於串流操作的具有型別欄位定義的 `StreamSchema` |
| **編解碼器擴充性 (Codec extensibility)** | 在 JS 和 Python 中支援自訂的型別對應 (例如 `Decimal`、`BigInt`、`Temporal`) |

## 下一步

- **[安裝 (Installation)](getting-started/installation.md)** — 在您的專案中設定 TTOON
- **[快速開始 (Quick Start)](getting-started/quick-start.md)** — 2 分鐘完成您的第一次來回轉換
- **[格式總覽 (Format Overview)](getting-started/format-overview.md)** — 了解 T-TOON 和 T-JSON 語法
- **[為何選擇 TTOON？ (Why TTOON?)](concepts/why-ttoon.md)** — 更深入的動機與使用案例
- **[API 矩陣 (API Matrix)](reference/api-matrix.md)** — 完整的跨語言 API 比較
