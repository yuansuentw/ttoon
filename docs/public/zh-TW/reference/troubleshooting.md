---
title: 疑難排解
sidebar_position: 7
sidebar_label: 疑難排解
description: 常見的 TTOON 錯誤以及其修復方法。
---

# 疑難排解

## 常見錯誤

| 錯誤 | 常見原因 | 解決方法 |
| :--- | :--- | :--- |
| `unknown bare token` | 在 `strict` 模式中使用了無引號的字串 | 加上雙引號：`"text"`，或改用 `compat` 模式 |
| `object keys must be strings` | T-JSON 的 key 缺少雙引號 | 使用 `{"key": 1}` 而非 `{key: 1}` |
| `line-separated rows are not supported` | 沒有表格標頭的純逗號分隔行 | 使用 T-TOON 表格 `[N]{fields}:` 或 T-JSON 陣列 |
| `read_arrow: input must be a list` | 將物件或純量傳給了 Arrow API | 改為統一物件列表結構 |
| `field 'x' has inconsistent types` | 同一欄位中有不同的純量型別 | 在輸入前統一欄位型別 |
| `Int64 value ... outside JS safe integer range` | JS `number` 無法安全表示 int64 | 使用 `intBigInt()` 或 `intNumber({ overflow: 'lossy' })` |
| `invalid escape sequence in T-TOON string` | 使用了不支援的 escape 序列（如 `\uXXXX`） | T-TOON 僅允許 `\\` `\"` `\n` `\r` `\t`；完整 escape 支援請使用 T-JSON |
| `unknown delimiter` / `unknown binary_format` | 傳入了不支援的選項值 | delimiter 僅接受 `","`, `"\t"`, `"|"`；binary_format 僅接受 `"hex"`, `"b64"` |

## 解析模式相關問題

### `strict` 模式拒絕無引號字串

```python
# 失敗 — "hello" 在 strict 模式下為未知的 bare token
ttoon.loads("key: hello", mode="strict")

# 成功 — 字串已加上引號
ttoon.loads('key: "hello"', mode="strict")
```

`strict` 模式適用於機器產生的資料；手動編寫的內容請使用 `compat` 模式。

### `mode` 主要影響 T-TOON

對批次解析與直接轉碼來說，`mode` 參數不影響 T-JSON 的結構解析，因為 T-JSON 依設計一律嚴格。不過在帶 schema 的 T-JSON 串流 reader 中，`mode` 會控制未知欄位是要略過（`compat`）還是報錯（`strict`）。

## Arrow 相關問題

### 並非所有資料都能使用 Arrow 路徑

`read_arrow()` 會拒絕以下情況：
- 根部不是列表（例如單一物件或純量）
- 物件欄位中包含巢狀列表或物件
- 同一欄位中的型別不一致（例如某些列是 `int`，其他列是 `string`）

### Datetime 時區混用

JS 的 Arrow 橋接器不允許在同一欄位中混用帶時區和不帶時區的 datetime。請確保同一欄位中的所有 datetime 要麼全部帶時區、要麼全部不帶。

## JS 特定問題

### `apache-arrow` 未安裝

`readArrow()`、`stringifyArrow()` 和 `stringifyArrowTjson()` 需要可選的對等依賴 `apache-arrow`：

```bash
npm install apache-arrow
```

### codec 未生效

- 透過 `use()` 註冊的 codec 是全域的，請確保 `await use(...)` 完成後再進行解析
- 單次呼叫時可在 `ParseOptions` 的 `codecs` 中覆寫全域設定，僅對該次呼叫生效
- codec 不影響 Arrow 路徑；它們作用於 JS 物件路徑，包括 `parse()`、`stringify()`、`toTjson()`，以及物件路徑的 streaming reader / writer

### TranscodeError 包裝

`tjsonToTtoon()` 和 `ttoonToTjson()` 會將底層錯誤包裝為 `TranscodeError`。在 JS 中，`e.phase` 目前一律回報為 `'parse'`，建議優先查看 `e.operation`、`e.sourceKind` 和 `e.source.message` 進行診斷：

```ts
try {
  tjsonToTtoon(text);
} catch (e) {
  if (e instanceof TranscodeError) {
    console.log(e.operation); // 'tjson_to_ttoon'
    console.log(e.phase);     // 在 JS 中目前一律為 'parse'
    console.log(e.sourceKind); // 底層的錯誤種類
  }
}
```

## Python 特定問題

### 未偵測到 Polars/PyArrow

`dumps()` 會自動偵測 Polars DataFrame 和 PyArrow Table/RecordBatch。若偵測失敗：

- 請確認 `polars` 和/或 `pyarrow` 已安裝
- 或直接將 Arrow 表格傳給 `dumps()` 或 `stringify_arrow_tjson()`

### `to_tjson()` 不接受 Arrow 輸入

Arrow/Polars → T-JSON 請使用 `stringify_arrow_tjson()`。`to_tjson()` 僅接受 Python 原生物件。

## 使用建議

| 情境 | 建議 |
| :--- | :--- |
| 一般物件交換 | 優先使用 T-TOON |
| 需要括號結構 | 使用 T-JSON |
| 大型表格與 DataFrame | 使用 Arrow / Polars 路徑 |
| JS 可能遇到 int64 | 上線前先決定 `bigint` 或 overflow 策略 |
| 人工編輯的資料 | 使用 `compat` 模式 |
| 機器產生的資料 | 使用 `strict` 模式 |
