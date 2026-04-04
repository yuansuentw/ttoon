---
title: T-JSON 批次 API
sidebar_position: 4
sidebar_label: T-JSON 批次 API
description: Python、JavaScript 與 Rust 的 T-JSON 批次讀寫與轉碼 API。
---

# T-JSON 批次 API

本頁整理的是會產生或處理 T-JSON 文字的非串流 API。可透過上方語言 tabs 在 Python、JavaScript 與 Rust 之間切換。

批次解析 API 依然會自動偵測 `T-TOON`、`T-JSON` 與 `typed_unit` 輸入。這裡的分類是依照 T-JSON 的使用情境來整理，不代表這些函式只能處理 T-JSON。

import Tabs from '@theme/Tabs';
import TabItem from '@theme/TabItem';

<Tabs groupId="language">
<TabItem value="python" label="Python">

套件：`ttoon`

## 讀取 T-JSON 批次文字

### `loads(text, mode=None) -> object`

- 將 `T-JSON`、`T-TOON` 或 `typed_unit` 解析為 Python 原生物件
- `mode` 只影響 T-TOON 解析路徑

### `read_arrow(text) -> pyarrow.Table`

- 將批次文字解析為 `pyarrow.Table`
- 自動偵測格式
- 輸入必須是由統一物件組成，且欄位為 scalar 的列表

## 寫出 T-JSON 批次文字

### `to_tjson(obj, binary_format=None) -> str`

- 將 Python 物件序列化為 T-JSON 文字
- 不接受 Arrow / Polars 輸入

### `stringify_arrow_tjson(obj, binary_format=None) -> str`

- 將 `pyarrow.Table`、`pyarrow.RecordBatch` 或 `polars.DataFrame` 序列化為 T-JSON list-of-objects

## 轉碼為 T-JSON

### `ttoon_to_tjson(text, *, mode="compat", binary_format=None) -> str`

- 僅透過 Rust IR 將 T-TOON 直接轉成 T-JSON
- `mode`：`"compat"`（預設）或 `"strict"`

## T-JSON 批次選項

| 參數 | APIs | 值 | 預設值 |
| :--- | :--- | :--- | :--- |
| `binary_format` | `to_tjson`, `stringify_arrow_tjson`, `ttoon_to_tjson` | `"hex"`, `"b64"` | `"hex"` |
| `mode` | `loads`, `ttoon_to_tjson` | `"compat"`, `"strict"` | `"compat"` |

## 相關工具

- `detect_format(text) -> str` 另見 [格式偵測](./format-detection.md)
- `TranscodeError` 的型別不會因目標批次格式不同而改變

</TabItem>
<TabItem value="js" label="JavaScript / TypeScript">

套件：`@ttoon/shared`

## 讀取 T-JSON 批次文字

### `parse<T>(text, options?): T`

- 將 `T-JSON`、`T-TOON` 或 `typed_unit` 解析為 JS 值
- `ParseOptions`：

| 屬性 | 型別 | 預設值 | 描述 |
| :--- | :--- | :--- | :--- |
| `mode` | `'compat' \| 'strict'` | `'compat'` | T-TOON / typed-unit 路徑的解析模式 |
| `codecs` | `CodecRegistry` | — | 單次呼叫的 codec 覆寫 |

### `readArrow(text): Promise<ArrowTable>`

- 將批次文字解析為 Arrow Table
- 自動偵測格式
- 需要 `apache-arrow`

## 寫出 T-JSON 批次文字

### `toTjson(value, options?): string`

- 將 JS 值序列化為 T-JSON 文字

`TjsonSerializeOptions`：

| 屬性 | 型別 | 預設值 | 描述 |
| :--- | :--- | :--- | :--- |
| `binaryFormat` | `'hex' \| 'b64'` | `'hex'` | 二進位編碼 |

### `stringifyArrowTjson(table, options?): Promise<string>`

- 將 Arrow Table 序列化為 T-JSON list-of-objects
- 需要 `apache-arrow`

## 轉碼為 T-JSON

### `ttoonToTjson(text, options?): string`

- 直接將 T-TOON 文字轉成 T-JSON
- `TtoonToTjsonOptions` 繼承自 `TjsonSerializeOptions`，另加 `mode?: ParseMode`

備註：JS 直接轉碼時，`TranscodeError.phase` 目前不可靠，較建議看 `sourceKind` 與底層 `source.message`。

## 相關工具

- `detectFormat(text)` 另見 [格式偵測](./format-detection.md)
- `toon.uuid()`、`toon.decimal()` 等 helpers 在把 JS 值序列化成 T-JSON 時同樣適用

</TabItem>
<TabItem value="rust" label="Rust">

Crate：`ttoon-core`

## 讀取 T-JSON 批次文字

```rust
fn from_ttoon(text: &str) -> Result<ir::Node>
fn from_ttoon_with_mode(text: &str, mode: ParseMode) -> Result<ir::Node>
fn read_arrow(text: &str) -> Result<ir::ArrowTable>
```

- 雖然函式名稱是歷史命名，但這些 API 同樣會自動偵測 T-JSON 輸入
- `read_arrow()` 可處理 T-TOON 與 T-JSON 的 arrowable 批次文字

## 寫出 T-JSON 批次文字

```rust
fn to_tjson(node: &ir::Node, opts: Option<&TjsonOptions>) -> Result<String>
fn arrow_to_tjson(table: &ir::ArrowTable, opts: Option<&TjsonOptions>) -> Result<String>
```

## 轉碼為 T-JSON

```rust
fn ttoon_to_tjson(text: &str, mode: ParseMode, opts: Option<&TjsonOptions>) -> Result<String>
```

## 設定型別

### `TjsonOptions`

```rust
pub struct TjsonOptions {
    pub binary_format: BinaryFormat,
}
```

### `ParseMode`

```rust
pub enum ParseMode {
    Strict,
    Compat,
}
```

### `BinaryFormat`

```rust
pub enum BinaryFormat { Hex, B64 }
```

## 相關工具

- `detect_format(input: &str)` 另見 [格式偵測](./format-detection.md)
- 共用的 schema 與 streaming 型別另見 [Stream API](./stream-api.md)

</TabItem>
</Tabs>

## 相關頁面

- **[T-TOON 批次 API](./ttoon-batch-api.md)** — 以 T-TOON 為中心的批次 API
- **[Stream API](./stream-api.md)** — 逐行讀寫 API
- **[typed value 參考](./typed-value-reference.md)** — 兩種批次語法共用的值層語意
