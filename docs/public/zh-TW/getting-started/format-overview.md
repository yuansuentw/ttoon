---
title: 格式總覽
sidebar_position: 3
sidebar_label: 格式總覽
description: T-TOON、T-JSON 與 typed value 基本規則的入門介紹。
---

# 格式總覽

TTOON 有兩種文字語法：

- **T-TOON**：以縮排表達結構，偏向人類閱讀與手動編輯
- **T-JSON**：以 `{}` / `[]` 表達結構，較接近既有 JSON 使用習慣
- **共用 typed value layer**：兩種語法使用相同的值層編碼規則

T-TOON 是建立在原始 [TOON](https://toonformat.dev/) 之上的擴充；而 TOON 來自 [toon-format 專案](https://github.com/toon-format/toon)。T-TOON 保留了 TOON 以縮排表達物件、以緊湊表格表達統一資料列的結構優勢，再往上增加明確的 typed value 與配套的 T-JSON 語法。

多數解析 API 都會自動偵測輸入格式，所以實務上通常可依可讀性與互操作需求選擇語法，而不是先設定 parser。

## T-TOON 一眼看懂

T-TOON 拿掉多餘括號，改用縮排表達結構。

### 物件

```text
name: "Alice"
age: 30
active: true
```

### 巢狀物件

```text
user:
  name: "Alice"
  address:
    city: "Taipei"
```

### 表格資料

```text
[2]{name,score}:
"Alice", 95
"Bob", 87
```

`[N]{fields}:` 標頭表示列數與欄位名稱，適合統一結構的物件陣列。

## T-JSON 一眼看懂

T-JSON 保留 JSON 風格的 `{}` / `[]` 結構，但值仍然使用 TTOON 的 typed syntax。

### 物件

```text
{"name": "Alice", "amount": 123.45m, "id": uuid(550e8400-e29b-41d4-a716-446655440000)}
```

### 陣列

```text
[1, 2, 3]
```

### 巢狀結構

```text
{"user": {"name": "Alice", "scores": [95, 87]}}
```

## Typed Values 一覽

兩種語法共用相同的 12 種內建 typed value：

| 型別 | 範例 |
| :--- | :--- |
| `null` | `null` |
| `bool` | `true` |
| `int` | `42` |
| `float` | `3.14` |
| `decimal` | `123.45m` |
| `string` | `"Alice"` |
| `date` | `2026-03-08` |
| `time` | `14:30:00` |
| `datetime` | `2026-03-08T14:30:00+08:00` |
| `uuid` | `uuid(550e8400-e29b-41d4-a716-446655440000)` |
| `hex` | `hex(48656C6C6F)` |
| `b64` | `b64(SGVsbG8=)` |

## 三個先記住的規則

- **字串一定要加引號**：使用 `"..."`，不要用 bare token
- **精確十進位用 `m` 後綴**：`123.45m` 是 decimal，`123.45` 是 float
- **UUID 與 binary 要用包裝器**：`uuid(...)`、`hex(...)`、`b64(...)`

## 該選哪一種語法？

| 情境 | 建議 |
| :--- | :--- |
| 人類可讀的設定檔、日誌、diff | T-TOON |
| 大型表格資料 | T-TOON tabular |
| 需與 JSON 風格系統整合 | T-JSON |
| 偏好括號式巢狀結構 | T-JSON |
| 跨語言物件交換 | 兩者皆可 |

## 下一步

- **[typed value 參考](../reference/typed-value-reference.md)** — 完整型別語意、詳細語法規則、跨語言對應、Arrow 對應與 RDBMS 對照
- **[格式偵測](../reference/format-detection.md)** — `tjson`、`ttoon`、`typed_unit` 的精確偵測規則
- **[T-TOON vs T-JSON](../concepts/ttoon-vs-tjson.md)** — 兩種語法的更完整比較
