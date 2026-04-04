# TTOON 文件索引

## 快速入門

- [介紹](intro.md) — 格式、SDK 與處理路徑的技術總覽
- [安裝](getting-started/installation.md) — 套件安裝指令與環境說明
- [快速開始](getting-started/quick-start.md) — 第一個 parse / stringify round trip
- [格式總覽](getting-started/format-overview.md) — T-TOON、T-JSON、typed value 與 tabular 語法
- 歡迎前往我們的官方網站[ttoon.dev](https://ttoon.dev/)

## 核心概念

- [為何選擇 TTOON？](concepts/why-ttoon.md) — 格式設計目標與適用情境
- [T-TOON vs T-JSON](concepts/ttoon-vs-tjson.md) — 結構與行為差異
- [解析模式](concepts/parse-modes.md) — `compat` 與 `strict`
- [object path vs Arrow path](concepts/object-path-vs-arrow-path.md) — 原生物件流程與 columnar 流程
- [效能](concepts/performance.md) — benchmark 解讀方式與效能模型

## 指南

- [Python](guides/python.md) — Python object path、Arrow path 與常見模式
- [JavaScript / TypeScript](guides/js-ts.md) — JS API、套件與執行環境說明
- [Rust](guides/rust.md) — crate 層級用法與選項
- [轉碼](guides/transcode.md) — `T-TOON` / `T-JSON` 直接轉換
- [Arrow and Polars](guides/arrow-and-polars.md) — tabular 與 columnar 工作流
- [Streaming](guides/streaming.md) — 逐列 reader / writer
- [JS Codecs and Int64](guides/js-codecs-and-int64.md) — JS codec、`BigInt` 與精度處理
- [Benchmarks](guides/benchmarks.md) — benchmark suite、dataset 與執行指令
- [貢獻者](guides/contributors.md) — public docs 同步與 release 準備

## 參考資料

- [Typed Value Reference](reference/typed-value-reference.md) — 12 種 typed type 與編碼規則
- [API Matrix](reference/api-matrix.md) — 跨語言 API 覆蓋情況
- [T-TOON 批次 API](reference/ttoon-batch-api.md) — 以 T-TOON 為中心的非串流 API
- [T-JSON 批次 API](reference/tjson-batch-api.md) — 以 T-JSON 為中心的非串流 API
- [Stream API](reference/stream-api.md) — stream reader、writer 與 schema 型別
- [格式偵測](reference/format-detection.md) — `ttoon`、`tjson`、`typed_unit` 偵測規則
- [行為與限制](reference/behaviors-and-limitations.md) — 邊界情況與明確限制
- [疑難排解](reference/troubleshooting.md) — 常見失敗模式與診斷方向
