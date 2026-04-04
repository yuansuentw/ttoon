---
title: 安裝
sidebar_position: 1
sidebar_label: 安裝
description: 在 Python、JavaScript/TypeScript 和 Rust 環境中安裝 TTOON 套件。
---

# 安裝

目前 `0.1.x` 版的 TTOON 套件已發布到 PyPI、npm 與 crates.io。官方文件網站為 [ttoon.dev](https://ttoon.dev/)。

import Tabs from '@theme/Tabs';
import TabItem from '@theme/TabItem';

<Tabs>
<TabItem value="python" label="Python">

```bash
pip install ttoon
```

若需要手動安裝原始碼發行包：

```bash
pip install ./ttoon-0.1.0.tar.gz
```

`pyarrow` 和 `polars` 已被宣告為套件依賴項。普通的 wheel 安裝不需要額外步驟。

目前的 Python 套件依賴於 `pyarrow>=23.0.0` 和 `polars>=1.37.1`。

如果您在一個精簡的環境中工作，請明確地安裝它們：

```bash
pip install pyarrow polars
```

需要 Python 3.11+。若安裝 wheel，Rust 核心已內建於 wheel 中；若安裝 sdist，則需要本地 Rust 工具鏈。

</TabItem>
<TabItem value="js" label="JavaScript / TypeScript">

```bash
npm install @ttoon/shared
```

若要進行 Arrow 表格操作，請加入可選的對等依賴項 (peer dependency)：

```bash
npm install @ttoon/shared apache-arrow
```

對於自訂的十進位 (decimal) codec，請安裝您的 codec 所使用的函式庫。常見的選擇包括 `decimal.js` 和 `big.js`：

```bash
npm install @ttoon/shared decimal.js
npm install @ttoon/shared big.js
```

> **注意**：`@ttoon/node` 和 `@ttoon/web` 是特定於環境的 `@ttoon/shared` 重新匯出。除非您需要明確區分執行環境，否則直接安裝 `@ttoon/shared` 即可。

```bash
npm install @ttoon/node
npm install @ttoon/web
```

JS SDK 使用 WASM 橋接來呼叫 Rust 核心引擎。WASM 模組已打包在套件內部 — 不需要額外的設定。

</TabItem>
<TabItem value="rust" label="Rust">

```bash
cargo add ttoon-core
```

`ttoon-core` crate 預設已包含 Apache Arrow 支援。

</TabItem>
</Tabs>

## 官方 SDK

所有 SDK 都共享同一套 Rust 核心，確保解析與序列化行為一致。JS 套件依執行環境拆分，但 `@ttoon/node` 和 `@ttoon/web` 本質上都只是 `@ttoon/shared` 的薄重新匯出層。

| 語言 | 套件 | 架構 |
| :--- | :--- | :--- |
| Python | `ttoon` | 透過 PyO3 的 Rust 核心 |
| JS / TS | `@ttoon/shared` | 透過 WASM 的 Rust 核心 |
| Node.js | `@ttoon/node` | 重新匯出 shared |
| Web | `@ttoon/web` | 重新匯出 shared |
| Rust | `ttoon-core` | 核心引擎 |

## 驗證安裝

<Tabs>
<TabItem value="python" label="Python">

```python
import ttoon
print(ttoon.dumps({"hello": "world"}))
# hello: "world"
```

</TabItem>
<TabItem value="js" label="JavaScript">

```ts
import { initWasm, stringify } from '@ttoon/shared';

await initWasm();
console.log(stringify({ hello: 'world' }));
// hello: "world"
```

</TabItem>
<TabItem value="rust" label="Rust">

```rust
use ttoon_core::{from_ttoon, to_ttoon};
let node = from_ttoon("hello: \"world\"").unwrap();
let text = to_ttoon(&node, None).unwrap();
assert_eq!(text, "hello: \"world\"\n");
```

</TabItem>
</Tabs>
