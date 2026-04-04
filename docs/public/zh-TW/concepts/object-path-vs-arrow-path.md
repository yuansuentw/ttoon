---
title: 物件路徑 vs Arrow 路徑
sidebar_position: 5
sidebar_label: 物件 vs Arrow 路徑
description: 了解 TTOON 中兩條獨立的處理路徑。
---

# 物件路徑 vs Arrow 路徑

TTOON 維護兩條獨立的處理路徑。了解何時該使用哪一條，是獲得最佳效能和開發者體驗的關鍵。

## 物件路徑

這是通用的路徑，用於在純文字和特定語言的原生物件之間進行轉換。

```text
T-TOON/T-JSON 文字 ──parse──→ IR ──convert──→ Python dict / JS 物件 / Rust Node
Python dict / JS 物件 / Rust Node ──convert──→ IR ──serialize──→ T-TOON/T-JSON 文字
```

**特性：**
- 適用於任何資料形狀 (物件、陣列、純量、巢狀結構)
- 產生熟悉的語言原生型別 (`dict`, `object`, `Node`)
- 以 IR (內部表示) 作為中間步驟
- 適用於中小型資料集、設定檔和一般用途的資料交換

**API：**

| 語言 | 解析 (Parse) | 序列化 T-TOON | 序列化 T-JSON |
| :--- | :--- | :--- | :--- |
| Python | `loads()` | `dumps(obj)` | `to_tjson(obj)` |
| JS | `parse()` | `stringify()` | `toTjson()` |
| Rust | `from_ttoon()` | `to_ttoon()` | `to_tjson()` |

## Arrow 路徑

這是專為表格資料設計的高效能路徑，可直接讀寫 Apache Arrow 列式 (columnar) 格式。

```text
T-TOON/T-JSON 文字 ──直接解析──→ Arrow 列式資料
Arrow 列式資料 ──直接序列化──→ T-TOON/T-JSON 文字
```

**特性：**
- 僅適用於表格資料 (由具有純量欄位的統一物件組成的列表)
- T-JSON 可以直接建立 Arrow Table / RecordBatch；T-TOON 表格目前仍使用相容路徑以進行轉換
- 盡可能實現零複製 (zero-copy)；記憶體分配降到最低
- 保留原生 Arrow 型別 (`Decimal128`, `Date32`, `FixedSizeBinary(16)`)
- 對於大型資料集而言，速度顯著更快且記憶體效率更高

**API：**

| 語言 | 解析 (Parse) | 序列化 T-TOON | 序列化 T-JSON |
| :--- | :--- | :--- | :--- |
| Python | `read_arrow()` | `dumps(table/df)` | `stringify_arrow_tjson()` |
| JS | `readArrow()` | `stringifyArrow()` | `stringifyArrowTjson()` |
| Rust | `read_arrow()` | `arrow_to_ttoon()` | `arrow_to_tjson()` |

## 何時該用哪一個

| 情境 | 路徑 | 原因 |
| :--- | :--- | :--- |
| 設定檔 | 物件 | 任意巢狀結構，體積小 |
| API 負載 (payloads) | 物件 | 通用用途，任意形狀 |
| 資料庫表匯出 | Arrow | 表格格式，可能很大 |
| Polars/Pandas 流水線 | Arrow | 已經是列式格式 |
| 串流處理大型資料集 | Arrow (串流) | 節省記憶體的逐行處理 |
| 跨語言物件交換 | 物件 | 熟悉的原生型別 |
| 分析流水線 (Analytics) | Arrow | 原生 Arrow 生態系統 |

## 串流變體

這兩條路徑都有用於逐行處理的串流變體：

| 路徑 | 串流讀取器 | 串流寫入器 |
| :--- | :--- | :--- |
| 物件 | `StreamReader` / `streamRead()` | `StreamWriter` / `streamWriter()` |
| Arrow | `ArrowStreamReader` / `streamReadArrow()` | `ArrowStreamWriter` / `streamWriterArrow()` |

另外還有每種格式的 T-JSON 變體 (例如 `TjsonStreamReader`, `TjsonArrowStreamWriter` 等等)。

詳細資訊請參閱 [串流指南](../guides/streaming.md)。

## 設計理念

這兩條路徑刻意保持獨立，而不是強制所有資料通過單一的流水線。這是因為：

1. **效能**：Arrow 列式讀寫可避免逐行 IR 轉換的開銷
2. **型別保真度**：保留原生的 Arrow 型別 (`Decimal128`, `FixedSizeBinary(16)`) 而不會發生有損轉換
3. **記憶體效率**：大型資料集永遠不會具現化為特定語言的原生物件樹
4. **可接受的程式碼重複**：路徑之間少量的共用邏輯是為了效能而做出的有意取捨
