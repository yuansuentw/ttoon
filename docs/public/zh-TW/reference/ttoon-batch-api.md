---
title: T-TOON 批次 API
sidebar_position: 3
sidebar_label: T-TOON 批次 API
description: Python、JavaScript 與 Rust 的 T-TOON 批次讀寫與轉碼 API。
---

# T-TOON 批次 API

本頁整理的是會產生或處理 T-TOON 文字的非串流 API。可透過上方語言 tabs 在 Python、JavaScript 與 Rust 之間切換。

批次解析 API 依然會自動偵測 `T-TOON`、`T-JSON` 與 `typed_unit` 輸入。這裡的分類是依照 T-TOON 的使用情境來整理，不代表這些函式只能處理 T-TOON。

import Tabs from '@theme/Tabs';
import TabItem from '@theme/TabItem';

<Tabs groupId="language">
<TabItem value="python" label="Python">

套件：`ttoon`

目前 Python 套件依賴 `pyarrow>=23.0.0` 與 `polars>=1.37.1`。

## 讀取 T-TOON 批次文字

### `loads(text, mode=None) -> object`

- 將 `T-TOON`、`T-JSON` 或 `typed_unit` 解析為 Python 原生物件
- `mode`：`"compat"`（預設）或 `"strict"`
- `mode` 只影響 T-TOON 解析路徑

### `read_arrow(text) -> pyarrow.Table`

- 直接將批次文字解析為 `pyarrow.Table`
- 自動偵測輸入格式
- 輸入必須是由統一物件組成，且欄位為 scalar 的列表

## 寫出 T-TOON 批次文字

### `dumps(obj, delimiter=",", indent_size=None, binary_format=None) -> str`

- 將 Python 原生物件序列化為 T-TOON 文字
- 也接受 `pyarrow.Table`、`pyarrow.RecordBatch`、`polars.DataFrame`
- Arrow / Polars 輸入會自動導向 Arrow path
- 統一物件列表會輸出為 `[N]{fields}:`

## 轉碼為 T-TOON

### `tjson_to_ttoon(text, *, delimiter=",", indent_size=None, binary_format=None) -> str`

- 僅透過 Rust IR 將 T-JSON 直接轉成 T-TOON
- 一律使用嚴格 T-JSON 解析
- 不接受 `mode` 參數

## T-TOON 批次選項

| 參數 | APIs | 值 | 預設值 |
| :--- | :--- | :--- | :--- |
| `delimiter` | `dumps`, `tjson_to_ttoon` | `","`, `"\t"`, `"|"` | `","` |
| `indent_size` | `dumps`, `tjson_to_ttoon` | `int \| None` | `None` |
| `binary_format` | 上述 serialize / transcode APIs | `"hex"`, `"b64"` | `"hex"` |
| `mode` | `loads` | `"compat"`, `"strict"` | `"compat"` |

## 相關工具

- `detect_format(text) -> str` 另見 [格式偵測](./format-detection.md)
- Python codec 註冊不影響 `loads()` 或批次轉碼；主要用在 streaming APIs

</TabItem>
<TabItem value="js" label="JavaScript / TypeScript">

套件：`@ttoon/shared`

## 讀取 T-TOON 批次文字

### `parse<T>(text, options?): T`

- 將 `T-TOON`、`T-JSON` 或 `typed_unit` 解析為 JS 值
- `ParseOptions`：

| 屬性 | 型別 | 預設值 | 描述 |
| :--- | :--- | :--- | :--- |
| `mode` | `'compat' \| 'strict'` | `'compat'` | T-TOON / typed-unit 路徑的解析模式 |
| `codecs` | `CodecRegistry` | — | 單次呼叫的 codec 覆寫 |

### `readArrow(text): Promise<ArrowTable>`

- 將批次文字解析為 Arrow Table
- 自動偵測格式
- 需要可選 peer dependency `apache-arrow`

## 寫出 T-TOON 批次文字

### `stringify(value, options?): string`

- 將 JS 值序列化為 T-TOON 文字
- 統一物件列表會自動輸出成 `[N]{fields}:`

`SerializeOptions`：

| 屬性 | 型別 | 預設值 | 描述 |
| :--- | :--- | :--- | :--- |
| `indentSize` | `number` | `2` | 縮排寬度 |
| `delimiter` | `',' \| '\t' \| '\|'` | `','` | 表格分隔符 |
| `binaryFormat` | `'hex' \| 'b64'` | `'hex'` | 二進位編碼 |

### `stringifyArrow(table, options?): Promise<string>`

- 將 Arrow Table 序列化為 T-TOON tabular 文字
- 需要 `apache-arrow`

### JS 型別標記

當你需要把 JS 中沒有原生型別的值序列化為 T-TOON 時，可使用 `toon` helpers：

```ts
import { toon } from '@ttoon/shared';

toon.uuid('550e8400-e29b-41d4-a716-446655440000');
toon.decimal('123.45');
toon.date('2026-03-08');
toon.time('14:30:00');
toon.datetime('2026-03-08T14:30:00+08:00');
```

## 轉碼為 T-TOON

### `tjsonToTtoon(text, options?): string`

- 直接將 T-JSON 文字轉成 T-TOON
- `TjsonToTtoonOptions` 繼承自 `SerializeOptions`

## 相關工具

- `detectFormat(text)` 另見 [格式偵測](./format-detection.md)
- Arrow helpers 與 stream schema types 另見 [Stream API](./stream-api.md)

</TabItem>
<TabItem value="rust" label="Rust">

Crate：`ttoon-core`

## 讀取 T-TOON 批次文字

```rust
fn from_ttoon(text: &str) -> Result<ir::Node>
fn from_ttoon_with_mode(text: &str, mode: ParseMode) -> Result<ir::Node>
fn read_arrow(text: &str) -> Result<ir::ArrowTable>
```

- `from_ttoon()` 會自動偵測格式，且明確使用 `ParseMode::Compat`
- `ParseMode::default()` 本身是 `ParseMode::Strict`
- `read_arrow()` 會自動偵測格式；輸入必須是統一物件列表

## 寫出 T-TOON 批次文字

```rust
fn to_ttoon(node: &ir::Node, opts: Option<&TtoonOptions>) -> Result<String>
fn arrow_to_ttoon(table: &ir::ArrowTable, opts: Option<&TtoonOptions>) -> Result<String>
```

## 轉碼為 T-TOON

```rust
fn tjson_to_ttoon(text: &str, opts: Option<&TtoonOptions>) -> Result<String>
```

- 一律使用嚴格 T-JSON 解析

## 設定型別

### `TtoonOptions`

```rust
pub struct TtoonOptions {
    pub binary_format: BinaryFormat,
    pub indent_size: u8,
    pub delimiter: Delimiter,
}
```

### `ParseMode`

```rust
pub enum ParseMode {
    Strict,
    Compat,
}
```

### `BinaryFormat` / `Delimiter`

```rust
pub enum BinaryFormat { Hex, B64 }
pub enum Delimiter { Comma, Tab, Pipe }
```

- `BinaryFormat::parse("hex") -> Option<BinaryFormat>`
- `Delimiter::parse(",") -> Option<Delimiter>`

## 相關工具

- `detect_format(input: &str)` 另見 [格式偵測](./format-detection.md)
- Streaming 與 schema 型別另見 [Stream API](./stream-api.md)

</TabItem>
</Tabs>

## 相關頁面

- **[T-JSON 批次 API](./tjson-batch-api.md)** — 以 T-JSON 為中心的批次 API
- **[Stream API](./stream-api.md)** — 逐行讀寫 API
- **[格式偵測](./format-detection.md)** — `ttoon`、`tjson`、`typed_unit` 的自動判斷規則
