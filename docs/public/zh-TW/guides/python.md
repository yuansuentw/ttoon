---
title: Python 指南
sidebar_position: 1
sidebar_label: Python
description: 使用 Python 操作 TTOON 的完整指南 — 包含批次處理、Arrow、Polars、串流與編解碼器。
---

# Python 指南

`ttoon` Python 套件透過 PyO3 包裝了 Rust 核心引擎。它在物件路徑上提供 Python 原生型別，並在表格路徑上提供零拷貝 (zero-copy) 的 Arrow 整合。

## 安裝

```bash
pip install ttoon

# 在最精簡或基於原始碼的環境中提供 Arrow / Polars 支援
pip install pyarrow polars
```

wheel 套件已經依賴 `pyarrow>=23.0.0` 和 `polars>=1.37.1`；額外的指令只需要在最簡的或基於原始碼安裝的環境中執行。

## 批次操作 (Batch Operations)

### 序列化：`dumps()`

```python
import datetime as dt
import decimal
import uuid
import ttoon

text = ttoon.dumps({
    "name": "Alice",
    "amount": decimal.Decimal("123.45"),
    "id": uuid.UUID("550e8400-e29b-41d4-a716-446655440000"),
    "created_at": dt.datetime(2026, 3, 8, 14, 30, 0),
})
```

`dumps()` 接受 Python 原生物件、`pyarrow.Table`、`pyarrow.RecordBatch` 和 `polars.DataFrame`。當給定 Arrow 或 Polars 輸入時，它會自動將其路由到高效能的 Arrow 路徑。

**選項：**

| 參數 | 型別 | 預設值 | 說明 |
| :--- | :--- | :--- | :--- |
| `delimiter` | `str` | `","` | 表格分隔符：`","`, `"\t"`, 或是 `"|"` |
| `indent_size` | `int \| None` | `None` | 縮排寬度；`None` 會使用 Rust 的預設值 (`2`) |
| `binary_format` | `str` | `"hex"` | 二進位編碼：`"hex"` 或是 `"b64"` |

### 反序列化：`loads()`

```python
data = ttoon.loads(text)
data = ttoon.loads(text, mode="strict")
```

能自動偵測格式 (T-TOON / T-JSON / typed unit)。`mode` 參數只會影響 T-TOON 格式的解析：

- `"compat"` (預設) — 未知的無引號標記將退化為字串
- `"strict"` — 未知的無引號標記會導致錯誤

### 產生 T-JSON：`to_tjson()`

```python
text = ttoon.to_tjson({
    "created_at": dt.datetime(2026, 3, 8, 14, 30, 0),
    "score": 12.5,
})
# {"created_at": 2026-03-08T14:30:00, "score": 12.5}
```

`to_tjson()` 不接受 Arrow/Polars 輸入。若是 Arrow → T-JSON，請使用 `stringify_arrow_tjson()`。

### Arrow 序列化：`stringify_arrow_tjson()`

```python
text = ttoon.stringify_arrow_tjson(table)
```

將 PyArrow Table 或 Polars DataFrame 序列化為 T-JSON 格式 (物件列表)。

### 格式偵測：`detect_format()`

```python
fmt = ttoon.detect_format(text)  # "ttoon" | "tjson" | "typed_unit"
```

## Arrow / Polars 路徑

### 序列化

```python
import polars as pl
import ttoon

df = pl.DataFrame({"name": ["Alice", "Bob"], "score": [95, 87]})
text = ttoon.dumps(df)
# [2]{name,score}:
# "Alice", 95
# "Bob", 87
```

`dumps()` 可偵測到 Polars DataFrames 和 PyArrow Tables，在內部會先將它們轉換為 Arrow，接著 Rust 便會直接從這列式資料進行序列化。

### 反序列化為 Arrow

```python
table = ttoon.read_arrow(text)  # 回傳 pyarrow.Table
```

輸入必須是統一物件的列表。欄位型別是從資料中推論而來的。結構性欄位 (list/object) 不能被 Arrow 化。

## 直接轉碼 (Direct Transcode)

在不具現化 Python 物件的情況下轉換格式：

```python
# T-JSON → T-TOON
ttoon_text = ttoon.tjson_to_ttoon(
    '{"name": "Alice", "scores": [95, 87]}',
    delimiter=",",
)

# T-TOON → T-JSON
tjson_text = ttoon.ttoon_to_tjson(
    'name: "Alice"\nage: 30',
    mode="compat",
)
```

文字只會經過 Rust IR — 所有的具型別語意都被完全保留。

## 註冊編解碼器 (Codec Registration)

註冊全域的編解碼器來客製化值的轉換方式：

```python
ttoon.use({
    "decimal": my_decimal_codec,
    "date": my_date_codec,
})
```

編解碼器的值可以是：

- 帶有 `"encode"` / `"decode"` 鍵 (他們對應的值為 callable) 的 Mapping
- 暴露了 `encode(value)` / `decode(value)` 方法的物件

每一個勾點 (hook) 都是選用的，但一個編解碼器必須提供至少一個可被呼叫的勾點。Python 的編解碼器只會影響物件路徑的串流讀取器與寫入器。它們不會改變 `loads()`、`to_tjson()`、Arrow 讀寫器或直接轉碼的 API。

## 串流 (Streaming)

若要進行逐行處理，請參閱 [串流指南 (Streaming Guide)](streaming.md)。Python 串流 API 支援 context manager 以及 Python 迭代器：

```python
import ttoon
from ttoon import StreamSchema, types

schema = StreamSchema({"name": types.string, "score": types.int})

# 寫入
with ttoon.stream_writer(sink, schema=schema) as writer:
    writer.write({"name": "Alice", "score": 95})
    writer.write({"name": "Bob", "score": 87})

# 讀取
for row in ttoon.stream_read(source, schema=schema):
    print(row)
```

## 錯誤處理

```python
from ttoon import TranscodeError

try:
    ttoon.tjson_to_ttoon(invalid_text)
except TranscodeError as e:
    print(e.operation)       # "tjson_to_ttoon" | "ttoon_to_tjson"
    print(e.phase)           # "parse" | "serialize"
    print(e.source_kind)     # 底層的來源種類
    print(e.source_message)  # 底層的來源訊息
    print(e.source)          # {"kind", "message", "span"}
```

解析錯誤中包含以供診斷用的行/欄 (line/column) 資訊。

## 下一步

- **[Arrow 與 Polars 指南](arrow-and-polars.md)** — 深入探索表格路徑
- **[串流指南](streaming.md)** — 逐行處理
- **[Python API 參考資料](../reference/python-api.md)** — 完整的 API 簽名
