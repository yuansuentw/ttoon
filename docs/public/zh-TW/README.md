# TTOON

TTOON 是一種為現代數據工作流程設計的**具型別純文字 (typed plain text)** 數據交換格式。它提供了兩種相輔相成的語法 — T-TOON (基於縮排) 和 T-JSON (基於括號) — 並提供 Python、JavaScript/TypeScript 和 Rust 的完整 SDK 支援。

## 文件

### 快速入門

- [介紹 (Introduction)](intro.md) — TTOON 是什麼以及它為何存在
- [安裝 (Installation)](getting-started/installation.md) — 在你的專案中設定 TTOON
- [快速開始 (Quick Start)](getting-started/quick-start.md) — 你的第一次來回轉換 (Round trip)
- [格式總覽 (Format Overview)](getting-started/format-overview.md) — 了解這兩種語法

### 核心概念

- [為何選擇 TTOON？ (Why TTOON?)](concepts/why-ttoon.md) — 動機、定位與使用案例
- [T-TOON vs T-JSON](concepts/ttoon-vs-tjson.md) — 詳細的語法比較
- [具型別值 (Typed Values)](concepts/typed-values.md) — 12 種具型別的值編碼
- [解析模式 (Parse Modes)](concepts/parse-modes.md) — `compat` 對比 `strict` 模式
- [物件路徑 vs Arrow 路徑 (Object Path vs Arrow Path)](concepts/object-path-vs-arrow-path.md) — 兩種處理路徑

### 指南

- [貢獻者 (Contributors)](guides/contributors.md) — 文件來源、同步流程與 release 準備
- [Benchmark](guides/benchmarks.md) — benchmark suite、dataset 與常用指令
- [Python](guides/python.md) — 完整的 Python 使用指南
- [JavaScript / TypeScript](guides/js-ts.md) — 完整的 JS/TS 使用指南
- [Rust](guides/rust.md) — 完整的 Rust 使用指南
- [轉碼 (Transcode)](guides/transcode.md) — 直接進行 T-JSON ↔ T-TOON 轉換
- [Arrow & Polars](guides/arrow-and-polars.md) — 高效能的表格路徑
- [串流 (Streaming)](guides/streaming.md) — 逐行讀取器與寫入器
- [JS 編解碼器與 Int64 (JS Codecs & Int64)](guides/js-codecs-and-int64.md) — 在 JS 中的自訂型別對應

### 參考資料

- [API 矩陣 (API Matrix)](reference/api-matrix.md) — 跨語言 API 比較 (18/18 一致性)
- [Python API](reference/python-api.md) — 完整的 Python API 參考
- [JS API](reference/js-api.md) — 完整的 JavaScript/TypeScript API 參考
- [Rust API](reference/rust-api.md) — 完整的 Rust API 參考
- [型別對應 (Type Mapping)](reference/type-mapping.md) — 跨語言型別轉換表
- [串流 Schema (Stream Schema)](reference/stream-schema.md) — 用於串流的 Schema 定義
- [格式偵測 (Format Detection)](reference/format-detection.md) — 格式自動偵測的運作方式
- [行為與限制 (Behaviors & Limitations)](reference/behaviors-and-limitations.md) — 邊界情況與約束
- [疑難排解 (Troubleshooting)](reference/troubleshooting.md) — 常見錯誤與修復方法

### 貢獻者

- [貢獻者指南](guides/contributors.md) — 維護者與文件貢獻者入口
- [Benchmark 指南](guides/benchmarks.md) — benchmark suite、dataset 自動下載與結果輸出
