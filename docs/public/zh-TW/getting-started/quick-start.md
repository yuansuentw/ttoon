---
title: 快速開始
sidebar_position: 2
sidebar_label: 快速開始
description: 在 5 分鐘內使用 TTOON 進行序列化、反序列化和轉碼。
---

# 快速開始

本指南涵蓋了最高效的使用路徑：

1. 序列化與反序列化物件
2. 產生 T-JSON 輸出
3. 處理表格資料與 Arrow
4. 在格式之間轉碼

## 1. 物件來回轉換

import Tabs from '@theme/Tabs';
import TabItem from '@theme/TabItem';

<Tabs>
<TabItem value="python" label="Python">

```python
import ttoon

data = {"name": "Alice", "age": 30, "id": "A-001"}

text = ttoon.dumps(data)
print(text)
# name: "Alice"
# age: 30
# id: "A-001"

restored = ttoon.loads(text)
```

</TabItem>
<TabItem value="js" label="JavaScript / TypeScript">

```ts
import { parse, stringify } from '@ttoon/shared';

const text = stringify({ name: 'Alice', age: 30, enabled: true });
const restored = parse(text);
```

</TabItem>
<TabItem value="rust" label="Rust">

```rust
use ttoon_core::{from_ttoon, to_ttoon};

let node = from_ttoon("name: \"Alice\"\nage: 30")?;
let text = to_ttoon(&node, None)?;
```

</TabItem>
</Tabs>

## 2. 產生 T-JSON

T-JSON 使用類似 JSON 的 `{}` / `[]` 括號，同時在值層面保留具型別的語法。

<Tabs>
<TabItem value="python" label="Python">

```python
import datetime as dt
import ttoon

text = ttoon.to_tjson({
    "created_at": dt.datetime(2026, 3, 8, 10, 30, 0),
    "score": 12.5,
})
print(text)
# {"created_at": 2026-03-08T10:30:00, "score": 12.5}
```

</TabItem>
<TabItem value="js" label="JavaScript / TypeScript">

```ts
import { toon, toTjson } from '@ttoon/shared';

const text = toTjson({
  id: toon.uuid('550e8400-e29b-41d4-a716-446655440000'),
  amount: toon.decimal('123.45'),
});
// {"id": uuid(550e8400-e29b-41d4-a716-446655440000), "amount": 123.45m}
```

JS 缺乏原生的 `Decimal` 和 `UUID` 型別，因此在序列化期間會使用 `toon.*()` 標記。

</TabItem>
</Tabs>

## 3. 表格資料與 Arrow

當資料是由統一物件組成的列表時，T-TOON 會自動輸出表格格式：

```text
[2]{name,score}:
"Alice", 95
"Bob", 87
```

<Tabs>
<TabItem value="python" label="Python: Polars / PyArrow">

```python
import polars as pl
import ttoon

df = pl.DataFrame({"name": ["Alice", "Bob"], "score": [95, 87]})

text = ttoon.dumps(df)
table = ttoon.read_arrow(text)  # 回傳 pyarrow.Table
```

- `dumps(df)` 內部會將其轉換為 Arrow，然後透過 Rust 核心進行序列化 — 沒有 `list[dict]` 中間產物
- `read_arrow()` 直接回傳一個 `pyarrow.Table`

</TabItem>
<TabItem value="js" label="JavaScript: Apache Arrow">

```ts
import { readArrow, stringifyArrow } from '@ttoon/shared';
import { tableFromArrays } from 'apache-arrow';

const table = tableFromArrays({
  name: ['Alice', 'Bob'],
  score: [95, 87],
});

const text = await stringifyArrow(table);
const restored = await readArrow(text);
```

</TabItem>
</Tabs>

## 4. 直接轉碼

在 T-JSON 和 T-TOON 之間轉換，無需具現化為特定語言的原生物件 — 文字僅通過 Rust IR。

<Tabs>
<TabItem value="python" label="Python">

```python
import ttoon

ttoon_text = ttoon.tjson_to_ttoon('{"name": "Alice", "scores": [95, 87]}')
tjson_text = ttoon.ttoon_to_tjson('name: "Alice"\nage: 30')
```

</TabItem>
<TabItem value="js" label="JavaScript / TypeScript">

```ts
import { tjsonToTtoon, ttoonToTjson } from '@ttoon/shared';

const ttoonText = tjsonToTtoon('{"name": "Alice", "age": 30}');
const tjsonText = ttoonToTjson('name: "Alice"\nage: 30');
```

</TabItem>
<TabItem value="rust" label="Rust">

```rust
use ttoon_core::{tjson_to_ttoon, ttoon_to_tjson, ParseMode};

let ttoon = tjson_to_ttoon(r#"{"key": 42}"#, None)?;
let tjson = ttoon_to_tjson("key: 42", ParseMode::Compat, None)?;
```

</TabItem>
</Tabs>

## 下一步

- **[格式總覽](format-overview.md)** — 了解這兩種語法和 typed value 系統
- **[Python 指南](../guides/python.md)** — 完整的 Python 使用指南
- **[JS/TS 指南](../guides/js-ts.md)** — 完整的 JavaScript/TypeScript 使用指南
- **[Arrow 與 Polars](../guides/arrow-and-polars.md)** — 深入了解高效能表格路徑
