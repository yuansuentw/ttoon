---
title: API 矩陣 (API Matrix)
sidebar_position: 1
sidebar_label: API 矩陣
description: Rust, JavaScript 和 Python 的跨語言 API 比較 — 18/18 對齊。
---

# API 矩陣 (API Matrix)

所有三個 TTOON SDK 都提供完全一致的 API 表面：在 Rust、JavaScript 和 Python 之間達到 **18/18** 的奇偶校驗 (parity)。

## 批次反序列化 (Batch Deserialization)

| 處理能力 | Rust | JavaScript | Python |
| :--- | :--- | :--- | :--- |
| 文字 → 物件/IR | `from_ttoon(text)` | `parse(text)` | `loads(text)` |
| 文字 → Arrow | `read_arrow(text)` | `readArrow(text)` | `read_arrow(text)` |

所有的批次反序列化 API 都會自動偵測輸入的格式 (T-TOON / T-JSON / typed unit)。

## 批次序列化 (Batch Serialization)

| 處理能力 | Rust | JavaScript | Python |
| :--- | :--- | :--- | :--- |
| 物件 → T-TOON | `to_ttoon(node)` | `stringify(value)` | `dumps(obj)` |
| 物件 → T-JSON | `to_tjson(node)` | `toTjson(value)` | `to_tjson(obj)` |
| Arrow → T-TOON | `arrow_to_ttoon(table)` | `stringifyArrow(table)` | `dumps(df/table)` |
| Arrow → T-JSON | `arrow_to_tjson(table)` | `stringifyArrowTjson(table)`| `stringify_arrow_tjson(table)` |

Python 的 `dumps()` 會自動偵測 Polars DataFrame 和 PyArrow Table/RecordBatch 輸入，並在內部將他們路由至 Arrow 路徑。

## 串流反序列化 (Streaming Deserialization)

| 處理能力 | Rust | JavaScript | Python |
| :--- | :--- | :--- | :--- |
| T-TOON → 物件 | `StreamReader` | `streamRead(source, opts)` | `stream_read(source, schema)` |
| T-JSON → 物件 | `TjsonStreamReader` | `streamReadTjson(source, opts)`| `stream_read_tjson(source, schema)`|
| T-TOON → Arrow | `ArrowStreamReader`| `streamReadArrow(source, opts)` | `stream_read_arrow(source, schema)` |
| T-JSON → Arrow | `TjsonArrowStreamReader`| `streamReadArrowTjson(source, opts)`| `stream_read_arrow_tjson(source, schema)`|

所有的串流讀取器都需要一個 `StreamSchema` 來定義欄位的名稱與型別。

## 串流序列化 (Streaming Serialization)

| 處理能力 | Rust | JavaScript | Python |
| :--- | :--- | :--- | :--- |
| 物件 → T-TOON | `StreamWriter` | `streamWriter(sink, opts)` | `stream_writer(sink, schema)` |
| 物件 → T-JSON | `TjsonStreamWriter`| `streamWriterTjson(sink, opts)`| `stream_writer_tjson(sink, schema)`|
| Arrow → T-TOON | `ArrowStreamWriter`| `streamWriterArrow(sink, opts)` | `stream_writer_arrow(sink, schema)` |
| Arrow → T-JSON | `TjsonArrowStreamWriter`| `streamWriterArrowTjson(sink, opts)`| `stream_writer_arrow_tjson(sink, schema)`|

所有的串流寫入器都需要一個 `StreamSchema`。

## 直接轉碼 (Direct Transcode)

| 處理能力 | Rust | JavaScript | Python |
| :--- | :--- | :--- | :--- |
| T-JSON → T-TOON | `tjson_to_ttoon(text)` | `tjsonToTtoon(text)` | `tjson_to_ttoon(text)` |
| T-TOON → T-JSON | `ttoon_to_tjson(text, mode)`| `ttoonToTjson(text)` | `ttoon_to_tjson(text)` |

轉碼只會經過 Rust 的內部表示 (IR) — 它不會具現化為特定語言的原生物件。所有的具備型別的語意 (decimal, uuid 等等) 皆會完全被保留下來。

## 實用工具 (Utilities)

| 處理能力 | Rust | JavaScript | Python |
| :--- | :--- | :--- | :--- |
| 格式偵測 | `detect_format(text)` | `detectFormat(text)` | `detect_format(text)` |
| 註冊編解碼器 | — (使用原生型別) | `use(codecs)` | `use(codecs)` |
| Schema 定義 | `StreamSchema` | `StreamSchema` | `StreamSchema` |

## 覆蓋率統計 (Coverage Statistics)

| 維度 | Rust | JavaScript | Python |
| :--- | :--- | :--- | :--- |
| 批次 (反序列化 + 序列化) | 6/6 | 6/6 | 6/6 |
| 串流 (反序列化 + 序列化) | 8/8 | 8/8 | 8/8 |
| 轉碼 | 2/2 | 2/2 | 2/2 |
| 實用工具 | 2/2 | 2/2 | 2/2 |
| **總計** | **18/18** | **18/18** | **18/18** |

編解碼器註冊被排除在 18/18 校驗的計算外，因為它是 JS/Python 專屬的；此矩陣只計算在三種 SDK 中都存在的 API。

## 架構注意事項 (Architecture Notes)

### 作為標準引擎的 Rust

Rust `ttoon-core` 是最標準的原生實作。Python 和 JavaScript SDK 都在背後委託這個 Rust 核心：

- **Python** — 透過 PyO3 的原生擴充 (原生的編譯進 wheel 中)
- **JavaScript** — 透過 WASM 的橋接器 (打包進了 npm 套件中)

這確保了完全一致的解析和序列化行為能夠跨平台執行。

### 串流格式慣例

- **T-TOON 串流** 使用 `[*]{fields}:` 作為無界限 (unbounded) 的表格標頭 (相較於 `[N]{fields}:` 這種宣告了固定行數的形式)。
- **T-JSON 串流** 使用一個最頂層的物件陣列，其中的純量值具有已知 schema。

### Arrow 路徑的最佳化

Rust 核心包含了一個為 T-JSON 輸入準備的直接 Arrow 路徑 (`tjson_arrow::read_arrow_tjson_direct`)，這可以跳過 Token/Node 的中介層，明顯降低大型資料集的記憶體消耗量。這個最佳化能透過共享的核心使所有的 SDK 都能因此受惠。
