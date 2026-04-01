---
title: Python API 參考資料 (Python API Reference)
sidebar_position: 2
sidebar_label: Python API
description: ttoon 套件的完整 Python API 參考資料。
---

# Python API 參考資料 (Python API Reference)

套件：`ttoon` (PyPI)

目前發布的 Python 套件依賴於 `pyarrow>=23.0.0` 和 `polars>=1.37.1`。

## 批次 API (Batch APIs)

### `dumps(obj, delimiter=",", indent_size=None, binary_format=None) → str`

將 Python 物件序列化為 T-TOON 文字。

- 接受：Python 原生物件、`pyarrow.Table`、`pyarrow.RecordBatch`、`polars.DataFrame`
- Arrow/Polars 的輸入將會自動導向至 Arrow 路徑
- 統一的物件列表將被輸出為表格格式 `[N]{fields}:`

### `loads(text, mode=None) → object`

將 T-TOON / T-JSON / 具型別單位 (typed unit) 文字反序列化為 Python 原生物件。

- 自動偵測格式
- `mode`：`"compat"` (預設) 或者是 `"strict"` — 此參數只影響 T-TOON 格式的解析

### `to_tjson(obj, binary_format=None) → str`

將 Python 物件序列化為 T-JSON 文字。

- **不**接受 Arrow/Polars 輸入 — 對此請改用 `stringify_arrow_tjson()`

### `stringify_arrow_tjson(obj, binary_format=None) → str`

將 PyArrow Table / RecordBatch 或是 Polars DataFrame 序列化為 T-JSON (物件列表) 格式。

### `read_arrow(text) → pyarrow.Table`

將 T-TOON / T-JSON 文字解析為 PyArrow Table。

- 自動偵測格式
- 輸入必須是由具有純量欄位的統一物件所組成的列表

### `detect_format(text) → str`

偵測輸入的格式。回傳 `"ttoon"`、`"tjson"` 或 `"typed_unit"`。

## 轉碼 API (Transcode APIs)

### `tjson_to_ttoon(text, *, delimiter=",", indent_size=None, binary_format=None) → str`

將 T-JSON 文字直接轉換為 T-TOON 文字 (僅透過 Rust IR)。

- 始終使用嚴格 (strict) T-JSON 解析 — 沒有 `mode` 參數

### `ttoon_to_tjson(text, *, mode="compat", binary_format=None) → str`

將 T-TOON 文字直接轉換為 T-JSON 文字 (僅透過 Rust IR)。

- `mode`：`"compat"` (預設) 或者是 `"strict"`

## 編解碼器 API (Codec API)

### `use(codecs) → None`

註冊全域的編解碼器 (codecs) 來為 Python 物件路徑的串流 API 自訂型別轉換方式。

每個編解碼器的值可以是：

- 帶有 `"encode"` 和/或 `"decode"` 鍵 (其對應的值為 callable) 的 Mapping
- 暴露了 `encode(value)` 和/或 `decode(value)` 方法的物件

必須提供至少一個可以被呼叫的勾點 (callable hook)。編解碼器會影響：

- `stream_read()` / `stream_writer()`
- `stream_read_tjson()` / `stream_writer_tjson()`

他們不會影響 `loads()`、`to_tjson()`、Arrow 路徑的串流處理或是直接轉碼功能。

```python
ttoon.use({"decimal": my_decimal_codec})
```

## 串流 API (Streaming APIs)

### 工廠函數 (Factory Functions)

| 函數 | 回傳型別 | 格式 | 路徑 |
| :--- | :--- | :--- | :--- |
| `stream_read(source, *, schema, mode=None, codecs=None)` | `StreamReader` | T-TOON | 物件 (Object) |
| `stream_read_tjson(source, *, schema, mode=None, codecs=None)` | `TjsonStreamReader` | T-JSON | 物件 (Object) |
| `stream_read_arrow(source, *, schema, batch_size=1024, mode=None)` | `ArrowStreamReader` | T-TOON | Arrow |
| `stream_read_arrow_tjson(source, *, schema, batch_size=1024, mode=None)` | `TjsonArrowStreamReader`| T-JSON | Arrow |
| `stream_writer(sink, *, schema, delimiter=",", binary_format=None, codecs=None)` | `StreamWriter` | T-TOON | 物件 (Object) |
| `stream_writer_tjson(sink, *, schema, binary_format=None, codecs=None)` | `TjsonStreamWriter` | T-JSON | 物件 (Object) |
| `stream_writer_arrow(sink, *, schema, delimiter=",", binary_format=None)` | `ArrowStreamWriter` | T-TOON | Arrow |
| `stream_writer_arrow_tjson(sink, *, schema, binary_format=None)` | `TjsonArrowStreamWriter` | T-JSON | Arrow |

### 串流讀取器類別 (Stream Reader Classes)

所有的讀取器都是 Python 的迭代器：

```python
for row in reader:
    print(row)  # 對應物件讀取器為 dict[str, Any]，應對 arrow 讀取器則是 RecordBatch
```

### 串流寫入器類別 (Stream Writer Classes)

所有的寫入器皆支援 context managers：

```python
with stream_writer(sink, schema=schema) as writer:
    writer.write({"name": "Alice", "score": 95})
result = writer.result  # StreamResult
```

| 類別 | 寫入方法 | 備註 |
| :--- | :--- | :--- |
| `StreamWriter` | `write(row: Mapping)` | 物件資料列 |
| `TjsonStreamWriter` | `write(row: Mapping)` | 物件資料列 |
| `ArrowStreamWriter` | `write_batch(batch)` | Arrow RecordBatch |
| `TjsonArrowStreamWriter` | `write_batch(batch)` | Arrow RecordBatch |

### `StreamResult`

由 `writer.close()` 回傳，或在關閉後透過 `writer.result` 存取。

| 屬性 | 型別 | 描述 |
| :--- | :--- | :--- |
| `rows_emitted` | `int` | 已寫入的列數 |

## Schema API

### `StreamSchema(fields)`

從 mapping 或包含 `(name, type_spec)` 成對數值的可迭代對象建立 schema。

```python
from ttoon import StreamSchema, types

schema = StreamSchema({
    "name": types.string,
    "score": types.int,
    "amount": types.decimal(10, 2),
    "active": types.bool.nullable(),
})
```

支援像 `Mapping` 一樣的存取方式：`schema["name"]`、`len(schema)`、以及疊代處理所有的欄位名稱。

驗證規則：

- 欄位名稱 (field names) 必須為 `str`
- 欄位規格 (field specs) 必須由 `ttoon.types` 建構而成
- 重複的欄位名稱將觸發 `ValueError`
- 空的 schemas 將觸發 `ValueError`

### `types` 命名空間

| 類型規格 (Type Spec) | 描述 |
| :--- | :--- |
| `types.string` | 字串 (String) |
| `types.int` | 整數 (Integer) |
| `types.float` | 浮點數 (Float) |
| `types.bool` | 布林 (Boolean) |
| `types.date` | 日期 (Date) |
| `types.time` | 時間 (Time) |
| `types.datetime` | 日期時間 (具備時區) |
| `types.datetime_naive` | 日期時間 (單純且無時區) |
| `types.uuid` | UUID |
| `types.binary` | 二進位 (Binary) |
| `types.decimal(precision, scale)` | 帶有特定精度和標度 (scale) 的 Decimal |

所有的型別規格都支援加上 `.nullable()` 來標示為可以為 null。

## 錯誤型別

### `TranscodeError`

發生轉換錯誤時，會由 `tjson_to_ttoon()` 或 `ttoon_to_tjson()` 拋出。

可用的屬性有：

| 屬性 | 型別 | 描述 |
| :--- | :--- | :--- |
| `operation` | `str` | `"tjson_to_ttoon"` 或 `"ttoon_to_tjson"` |
| `phase` | `str` | `"parse"` 或是 `"serialize"` |
| `source_kind` | `str` | 造成問題的原始來源錯誤種類 |
| `source_message` | `str` | 底層的來源錯誤訊息 |
| `source` | `dict` | `{"kind", "message", "span"}`，在這之中 `span` 會是 `None` 或者是 `{"offset", "line", "column"}` |

## 序列化選項 (Serialization Options)

| 參數 | APIs | 值 | 預設值 |
| :--- | :--- | :--- | :--- |
| `delimiter` | `dumps`, `tjson_to_ttoon`, 串流寫入器 | `","`, `"\t"`, `"|"` | `","` |
| `indent_size` | `dumps`, `tjson_to_ttoon` | `int \| None` (Rust 有效範圍: `0..=255`) | `None` |
| `binary_format` | 所有 serialize / transcode APIs | `"hex"`, `"b64"` | `"hex"` |
| `mode` | `loads`, `ttoon_to_tjson`, 串流讀取器 | `"compat"`, `"strict"` | `"compat"` |
