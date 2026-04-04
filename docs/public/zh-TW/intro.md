---
title: 介紹 (Introduction)
sidebar_position: 1
sidebar_label: 介紹
description: TTOON 格式、SDK 與處理路徑的技術總覽。
---

# TTOON

TTOON 是一套具型純文字資料交換系統，在同一專案下提供兩種互補語法：

- **T-TOON**：基於縮排，從 TOON v3.0 延伸
- **T-JSON**：維持 JSON 結構，僅在 leaf value 層使用相同的 typed value 系統

它們不是兩個獨立產品，而是建立在同一套 typed model 與同一個 Rust core 上的兩種序列化語法。

## 格式模型

| 語法 | 結構 | 適合情境 |
| :--- | :--- | :--- |
| `T-TOON` | 縮排式，支援 tabular `[N]{fields}:` 區塊 | 可讀性優先的資料與表格資料集 |
| `T-JSON` | JSON 結構，leaf value 可帶 typed 語意 | 需要 JSON 風格容器的整合場景 |

兩種語法共用同一組 typed values。目前支援的 typed set 包含：

- `null`、`bool`、`int`、`float`、`decimal`、`string`
- `date`、`time`、`datetime`
- `uuid`、`hex`、`b64`

實際編碼規則與範例請見 [Typed Value Reference](reference/typed-value-reference.md)。

## SDK 與共用核心

| 語言 | 套件 | 架構 |
| :--- | :--- | :--- |
| Python | `ttoon` | 透過 PyO3 橋接 Rust 核心 |
| JavaScript / TypeScript | `@ttoon/shared` | 透過 WASM 橋接 Rust 核心 |
| JavaScript / Node.js | `@ttoon/node` | 重新匯出 `@ttoon/shared` |
| JavaScript / Web | `@ttoon/web` | 重新匯出 `@ttoon/shared` |
| Rust | `ttoon-core` | 核心引擎 (標準實作) |

Python、JavaScript 與 Rust 的解析與序列化語意都由同一個 Rust core 提供。公開 API 範圍請見 [API Matrix](reference/api-matrix.md)。

## 處理路徑

TTOON 在各語言 SDK 中提供兩條主要執行路徑：

- **object path**：進出語言原生物件
- **Arrow path**：直接處理 columnar 資料，不先具現化成逐列物件

API 也分成兩種 I/O 風格：

- **batch API**：整份文件或整張 table 的 parse / stringify
- **stream API**：以 schema 驅動的逐列 reader / writer

相關頁面：

- [object path vs Arrow path](concepts/object-path-vs-arrow-path.md)
- [T-TOON 批次 API](reference/ttoon-batch-api.md)
- [T-JSON 批次 API](reference/tjson-batch-api.md)
- [Stream API](reference/stream-api.md)

## 最小範例

```ttoon
user:
  id: uuid(550e8400-e29b-41d4-a716-446655440000)
  name: "Alice"
  joined: 2026-03-08
  balance: 123.45m
tags: [2]:
  - "alpha"
  - "beta"
```

這份文件在 round trip 後，型別不會全部退化成字串：

- `uuid(...)` 仍保留 UUID 語意，而非普通字串標記
- `2026-03-08` 仍是 date
- `123.45m` 仍是 decimal
- tabular 與 Arrow path 可保留資料集的 columnar 語意

## 建議先讀

- **[安裝](getting-started/installation.md)** — 套件安裝與環境設定
- **[快速開始](getting-started/quick-start.md)** — 各語言的第一個端到端範例
- **[格式總覽](getting-started/format-overview.md)** — 精確語法與 typed value 規則
- **[Typed Value Reference](reference/typed-value-reference.md)** — 各型別的編碼細節
- **[API Matrix](reference/api-matrix.md)** — 跨語言 API 比較
