---
title: Arrow & Polars
sidebar_position: 5
sidebar_label: Arrow & Polars
description: 整合 Apache Arrow 與 Polars 的高效能表格資料路徑。
---

# Arrow & Polars 指南

TTOON 維護兩條獨立的處理路徑：**物件路徑 (object path)** (通用) 和 **Arrow 路徑** (高效能表格處理)。本指南涵蓋了 Arrow 路徑。

## 為何需要獨立的 Arrow 路徑？

Arrow 路徑會盡量讓表格資料維持在 Arrow 原生的列式形式，而不是先轉成語言原生物件。現階段最完整的快速路徑是 T-JSON → Arrow 的直接讀取；T-TOON tabular 仍會透過相容的 `Node` 路徑與 Arrow 互通。對表格資料而言，這代表：

- **不會在 Arrow 端具現化為語言原生的逐列物件** — 資料維持 columnar，而不是先變成 `dict` / JS object rows
- **在有 direct path 的情況下可降低轉換成本** — 尤其是 T-JSON → Arrow 讀取
- **保留原生型別** — `Decimal128`, `Date32`, `Timestamp`, `FixedSizeBinary(16)` (UUID) 皆保持在他們原生的 Arrow 形式

## Python: Polars & PyArrow

### 序列化

```python
import polars as pl
import pyarrow as pa
import ttoon

# Polars DataFrame
df = pl.DataFrame({"name": ["Alice", "Bob"], "score": [95, 87]})
text = ttoon.dumps(df)
# [2]{name,score}:
# "Alice", 95
# "Bob", 87

# PyArrow Table
table = pa.table({"name": ["Alice", "Bob"], "score": [95, 87]})
text = ttoon.dumps(table)

# Arrow → T-JSON
text = ttoon.stringify_arrow_tjson(df)
# [{"name": "Alice", "score": 95}, {"name": "Bob", "score": 87}]
```

`dumps()` 會自動偵測 Polars DataFrame 與 PyArrow Table/RecordBatch 輸入，並將他們路由至 Arrow 路徑。Polars DataFrames 會被優先轉換為 Arrow (在 Polars 中為 zero-copy)。

### 反序列化為 Arrow

```python
table = ttoon.read_arrow(text)  # 回傳 pyarrow.Table
```

從回傳的 `pyarrow.Table` 中，您可以轉換為任何下游格式：

```python
df = pl.from_arrow(table)      # Polars DataFrame
pandas_df = table.to_pandas()  # Pandas DataFrame
```

### 分隔符號選項

```python
text = ttoon.dumps(df, delimiter="|")
# [2]{name,score}:
# "Alice"| 95
# "Bob"| 87

text = ttoon.dumps(df, delimiter="\t")
```

## JavaScript: Apache Arrow

需要安裝可選的 peer dependency `apache-arrow`。

### 序列化

```ts
import { stringifyArrow, stringifyArrowTjson } from '@ttoon/shared';
import { tableFromArrays } from 'apache-arrow';

const table = tableFromArrays({
  name: ['Alice', 'Bob'],
  score: [95, 87],
});

// Arrow → T-TOON 表格
const ttoonText = await stringifyArrow(table);

// Arrow → T-JSON
const tjsonText = await stringifyArrowTjson(table);
```

### 反序列化為 Arrow

```ts
import { readArrow } from '@ttoon/shared';

const table = await readArrow(text);
```

JS 中的 Arrow API 為 `async` (非同步) 的，因為它們會動態匯入 `apache-arrow` 模組。

## Rust

```rust
use ttoon_core::{read_arrow, arrow_to_ttoon, arrow_to_tjson};

let table = read_arrow(text)?;
let ttoon = arrow_to_ttoon(&table, None)?;
let tjson = arrow_to_tjson(&table, None)?;
```

## Arrow 來源的輸入限制

多個語言的 `read_arrow()` 都會強制執行下列限制：

| 條件 | 描述 |
| :--- | :--- |
| 根部必須為列表 | Arrow 橋接器只處理表格資料 |
| 每個元素必須為物件 | 物件的鍵 (key) 將會成為 schema 的欄位 |
| 欄位型別必須一致 | 不可在同一列 (column) 中混用不同的純量型別 |
| 不能有結構性欄位 | list/object 的值不能被轉換為 Arrow |

## Arrow Schema 對應

| typed type | Arrow 型別 |
| :--- | :--- |
| `int` | `Int64` |
| `float` | `Float64` |
| `decimal` | `Decimal128` 或 `Decimal256` (取決於精度) |
| `string` | `Utf8` |
| `bool` | `Boolean` |
| `date` | `Date32` |
| `time` | `Time64(Microsecond)` |
| `datetime` | `Timestamp(Microsecond[, tz])` |
| `uuid` | `FixedSizeBinary(16)` + UUID metadata |
| `hex`/`b64` | `Binary` |
| `null` | 允許為 null 的列；全為 null 將被推論為 `Null` |

Arrow 類型會以其原生的解析度被保存 — `decimal` 不會被降級成 string，`uuid` 是使用 `FixedSizeBinary(16)` 及元資料 (metadata) 所構成的。

## 效能筆記

### T-JSON 的直接路徑

在 Rust 核心內部包含一個用於 T-JSON → Arrow 的兩次巡訪 (two-pass) 直接路徑 (`read_arrow_tjson_direct`)，它跳過了 Token/Node 的中介層。在面臨龐大的資料集這能顯著的降低記憶體的使用，並透過共享核心來使所有的 SDK 受益。

### 支援稀疏 Schema

T-JSON 的 `read_arrow()` 支援稀疏行 (sparse rows) — 缺失的鍵會被視為 null。Schema 欄位的順序是從批次內第一筆匹配的順序推論而來。

T-TOON 表格的欄位順序及寬度則是直接取自標頭 (header) 本身。

### Datetime 時區的一致性

JS Arrow bridge 並不允許在同一列 (column) 中混雜包含時區以及不包含時區 (naive) 的 datetimes。混雜使用就會導致 schema 推論錯誤。

## 下一步

- **[串流指南](streaming.md)** — 使用 `ArrowStreamReader` / `ArrowStreamWriter` 進行逐行的 Arrow 串流
- **[型別對應](../getting-started/format-overview.md)** — 完整的跨語言型別對應表
- **[Stream API](../reference/stream-api.md)** — 串流 API 與 schema 定義
