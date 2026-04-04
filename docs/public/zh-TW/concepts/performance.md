---
title: 性能特性
sidebar_position: 6
sidebar_label: 性能特性
description: 處理路徑、記憶體概況、同步 API 設計，以及實務上的資料量選擇建議。
---

# 性能特性

本頁彙整三種處理模式 — **物件路徑 (object)**、**Arrow 路徑**和**串流 (streaming)** — 的運行特性，並提供根據資料量選擇合適模式的實務建議。

## 物件路徑的轉換開銷

物件路徑透過 IR（中間表示層）在文字和語言原生物件之間轉換：

```text
text → parse → IR → convert → dict / object
dict / object → convert → IR → serialize → text
```

每個值都會單獨跨越 FFI 邊界。以 Python 為例，pyo3 需要對每個葉值依序嘗試型別檢查（datetime → date → time → uuid → decimal → bool → int → float → string）。JavaScript 透過 WASM bridge 也有類似的逐值開銷。

序列化方向開銷最大，因為每個值都必須經過檢查、標記和轉換後，Rust 核心才能進行格式化。相對於 Rust 核心基準，Python 物件序列化約慢 **12–28 倍**，JavaScript 約慢 **17–18 倍**，因為逐值處理時 FFI 開銷佔主導。

對於設定檔、API 酬載或大約 **10 K 筆以下**的資料集，此開銷可以忽略。超過此規模的表格資料，建議改用 Arrow 路徑。

## Arrow 列式優勢

Arrow 路徑避免建立語言原生物件樹，並盡量讓資料維持在 Arrow 的列式形式。現階段真正不經 IR 的最強路徑是 T-JSON → Arrow 讀取；T-TOON tabular 仍會先走相容路徑再轉為 Arrow：

```text
text → Rust 核心 → Arrow IPC bytes → 零複製 table
Arrow table → IPC bytes → Rust 核心 → text
```

由於 Arrow 端不需要逐筆轉成語言原生物件，Arrow 路徑對等量的表格資料比物件路徑**快 7–23 倍**。Python 和 Rust 的吞吐量幾乎相同，因為跨越 FFI 邊界的只有一個 IPC buffer — 核心引擎負責所有繁重的工作。

Arrow 型別被原生保留（`Decimal128`、`Date32`、`Timestamp`、UUID 對應的 `FixedSizeBinary(16)`），不會退化為字串。

## 串流的記憶體用量

批次模式將整個資料集具現化至記憶體。串流則逐行（物件路徑）或按 record batch（Arrow 路徑）處理，只持有當前的區塊。

在大規模資料下，串流大幅降低峰值記憶體：

- **100 K 筆** — 串流的峰值記憶體約為批次的 **1/5**
- **1 M 筆** — 串流約為批次的 **1/20**，因為批次記憶體隨資料量線性增長，而串流受限於 chunk 大小

串流反序列化也比批次**快 30–60 %**，因為它避免了建構單一大型記憶體配置。串流序列化的吞吐量與批次大致相同，但峰值記憶體約節省 **40 %**。

**經驗法則：**

| 資料量 | 建議模式 |
| :--- | :--- |
| < 10 K 筆 | 批次 — 根據資料結構選擇物件或 Arrow |
| 10 K – 100 K 筆 | 表格資料用 Arrow 批次；巢狀結構用物件批次 |
| > 100 K 筆 | 表格資料用 Arrow 串流 |

## 同步 API 設計

所有批次和轉碼函式（`parse`、`stringify`、`toTjson`、`tjsonToTtoon`、`ttoonToTjson`）在每個 SDK 中皆為**同步呼叫**。核心引擎執行的是 CPU 密集型的文字處理 — 解析、型別推斷、序列化 — 皆在單次傳遞中完成且無 I/O 等待，因此同步呼叫是最自然的設計。

唯一的非同步 API 是那些在邊界處需要動態模組載入的情境：

| API | 非同步原因 |
| :--- | :--- |
| `initWasm()` (JS) | 擷取並實例化 WASM 二進位 |
| `readArrow()` / `stringifyArrow()` / `stringifyArrowTjson()` (JS) | 動態 import `apache-arrow` |
| 串流讀取器 / 寫入器 | 本質上為增量式；每個區塊 yield/await |

如果您正在建構基於 TTOON 的管線，可以在緊湊迴圈中直接呼叫 `parse` / `stringify` / `toTjson`，無需擔心 Promise 開銷或事件迴圈阻塞（超出呼叫本身的執行時間）。

## 總結

| | 物件路徑 | Arrow 路徑 | Arrow 串流 |
| :--- | :--- | :--- | :--- |
| **資料形狀** | 任意 | 僅表格 | 僅表格 |
| **FFI 成本** | 逐值 | 逐 IPC buffer | 逐 batch chunk |
| **適用規模** | < 10 K 筆，巢狀 | 10 K – 100 K 筆 | > 100 K 筆 |
| **記憶體** | 與資料量成正比 | 與資料量成正比 | 受限於 chunk 大小 |
| **同步 / 非同步** | 同步 | 同步（JS：非同步邊界） | 非同步（增量式） |
