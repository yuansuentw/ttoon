---
title: 為何選擇 TTOON？
sidebar_position: 1
sidebar_label: 為何選擇 TTOON？
description: TTOON 作為具型別純文字資料交換格式的動機、定位與使用案例。
---

# 為何選擇 TTOON？

## 問題所在

在系統之間交換結構化資料通常會迫使我們做出取捨：

| 格式 | 人類可讀 | 具型別 | 高效能 |
| :--- | :--- | :--- | :--- |
| JSON | 中等 | 否 — 所有東西都是字串/數字/布林值/null | 否 |
| CSV | 是 | 否 — 所有儲存格都是字串 | 否 |
| YAML | 是 | 有限 — 隱含型別強制轉換 (implicit type coercion) | 否 |
| Protocol Buffers | 否 (二進位) | 是 | 是 |
| Apache Arrow IPC | 否 (二進位) | 是 | 是 |
| Parquet | 否 (二進位) | 是 | 是 |

JSON 無處不在，但無法區分 `decimal` 和 `float`，沒有 `date`/`time`/`uuid`/`binary` 型別，且在大型整數上會遺失精度。CSV 是扁平的且完全無型別的。YAML 嘗試進行隱含型別強制轉換 (惡名昭彰的 `"no"` → `false` 問題)。二進位格式解決了型別問題，但完全犧牲了人類可讀性。

## 為何不是直接只用 Arrow？

Arrow 本來就很適合作為不同資料庫、不同語言之間的共通中介格式。它具備明確 schema、良好的 columnar 記憶體模型，以及高效批次處理能力；在大部分純機器處理場景中，仍然建議優先直接使用 Arrow。

問題在於 Arrow 主要是 binary format。當資料需要被列印、顯示、人工檢查、貼進文件或 prompt，或交給 AI agent 閱讀時，各語言與各套件對 Arrow 的顯示方式並不一致，而且像 `float` 與 `decimal` 這種差異，人類通常無法從一般輸出結果直接分辨。

TTOON 的角色就是補上這一層：為一組資料庫常見、跨語言也相對穩定的通用型別，定義統一的純文字序列化表示法，讓同一份資料在顯示、保存、傳輸與人工/AI 閱讀時，都能維持清楚且可預測的型別語意。

因此，本專案不只定義序列化格式，也一併實作對應的反序列化邏輯，讓資料可以在純文字格式、各語言原生物件與 Arrow 之間雙向 round-trip。TTOON 不只是「給人看的 Arrow 印出結果」，而是可實際作為保存與傳輸載體的 typed plain text format。實務上的分工很簡單：機器對機器優先用 Arrow；需要以文字被閱讀時再用 TTOON。

## TTOON 提供什麼

TTOON 填補了這個空白：**具型別、人類可讀、純文字且高效能**。

- **12 種明確的具型別編碼** — `null`、`bool`、`int`、`float`、`decimal`、`string`、`date`、`time`、`datetime`、`uuid`、`hex`、`b64` — 全部都沒有歧義，也沒有隱含的強制轉換。
- **兩種相輔相成的語法** — 為了可讀性和表格資料設計的 T-TOON；以及為了相容 JSON 生態系統設計的 T-JSON。
- **跨語言的保真度** — Python `Decimal` ↔ Rust `Decimal128` ↔ JS `string`/`Decimal.js` 皆透過相同的 `123.45m` 編碼傳遞，保留了精度和意圖。
- **在可用的情況下與 Arrow 整合並提供直接的快速路徑** — T-JSON 表格讀取使用專用的 Arrow 路徑，而 T-TOON 仍然透過共用的 IR 路徑來支援與 Arrow 的互操作性。
- **串流** — 這兩種格式均支援逐行讀取器與寫入器，適用於大型資料集與即時流水線。

## 與 TOON 的關係

TTOON 並不是憑空發明的新格式。它的 T-TOON 語法是建立在 [TOON](https://toonformat.dev/) 之上的擴充，而原始專案即是 [toon-format/toon](https://github.com/toon-format/toon)。

TTOON 刻意保留了 TOON 已經做得很好的部分：

- 使用縮排表達結構，方便人類快速掃讀
- 對統一陣列提供緊湊的 tabular layout
- 維持適合 prompt、diff 與人工檢視的純文字特性

在這個基礎上，TTOON 再往上增加明確的 typed value 語意、跨語言執行期對應、Arrow-native 互操作，以及括號式的配套語法 T-JSON。

## 使用案例

### 跨語言資料交換

Python 資料科學流水線 → TTOON 文字 → Rust 處理引擎 → TTOON 文字 → JS 儀表板。型別在每次傳遞都能保留下來。

### 資料庫匯出與稽核

將資料庫的表匯出為人類可讀且具備型別的純文字，便於版本控制的比對和合規性稽核。與 CSV 不同的是，`decimal(10,2)` 欄位能保持是十進位，而不會截斷為浮點數。

### Arrow / Polars 分析流水線

在基於 Arrow 的流程中的任何一點注入或提取具型別的純文字。 T-JSON 表格輸入有專用的 Arrow 讀取路徑；T-TOON 表格輸入仍然有效，但目前在轉換為 Arrow 之前會先透過共用的 IR 相容路徑進行解析。

### 結構化日誌記錄

創建可以同時被人類閱讀且能被機器解析，並保留型別語意的日誌項目。

### LLM / AI 資料流水線

作為 LLM 輸出與下游處理之間的型別安全中介 — 以防止基於 JSON 的 LLM 流程中常見隱含型別轉換導致的錯誤。

## 設計哲學

1. **顯式優於隱式 (Explicit over implicit)** — 沒有型別強制轉換。`123.45m` 永遠是十進位數，`123.45` 永遠是浮點數。
2. **基於 Rust 且跨語言一致** — 單一 Rust 核心引擎驅動所有 SDK，確保一致的行為。
3. **兩條路徑，互相獨立** — 用於一般用途的物件路徑 (Object path)；以及用於提升列式效能的 Arrow 路徑 (Arrow path)。他們互不影響。
4. **解析就是驗證 (Parse is validation)** — 沒有獨立的 `validate()` 步驟。如果它能解析，它就是有效的。
5. **格式路由不退回 (Format routes don't fallback)** — 一旦解析器確定是 T-TOON 或 T-JSON，就會持續到底。沒有靜默的重試機制。
