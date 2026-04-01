---
title: 安裝 (Installation)
sidebar_position: 1
sidebar_label: 安裝
description: 在 Python、JavaScript/TypeScript 和 Rust 環境中安裝 TTOON 套件。
---

# 安裝 (Installation)

目前 `0.1.x` 版的 TTOON 套件**不發布到 PyPI、npm 或 crates.io**。public release 只提供本地安裝用的 package artifacts；請先從 GitHub Release 或 GitHub Actions artifacts 下載對應檔案，再用本地檔案安裝。

## Python

請先下載 `python-wheel-*` 或 `python-sdist` artifact。

```bash
pip install ./ttoon-0.1.0-*.whl
```

若只有 source distribution：

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

## JavaScript / TypeScript

請先下載 `js-packages` artifact，內容會包含 `ttoon-shared-0.1.0.tgz`、`ttoon-node-0.1.0.tgz`、`ttoon-web-0.1.0.tgz`。

```bash
npm install ./ttoon-shared-0.1.0.tgz
```

若要進行 Arrow 表格操作，請加入可選的對等依賴項 (peer dependency)：

```bash
npm install ./ttoon-shared-0.1.0.tgz apache-arrow
```

對於自訂的十進位 (decimal) 編解碼器，請安裝您的編解碼器所使用的函式庫。常見的選擇包括 `decimal.js` 和 `big.js`：

```bash
npm install ./ttoon-shared-0.1.0.tgz decimal.js
npm install ./ttoon-shared-0.1.0.tgz big.js
```

> **注意**：`@ttoon/node` 和 `@ttoon/web` 是特定於環境的 `@ttoon/shared` 重新匯出。若要安裝它們，請一併安裝對應的 `ttoon-shared-0.1.0.tgz`。

```bash
npm install ./ttoon-shared-0.1.0.tgz ./ttoon-node-0.1.0.tgz
npm install ./ttoon-shared-0.1.0.tgz ./ttoon-web-0.1.0.tgz
```

JS SDK 使用 WASM 橋接來呼叫 Rust 核心引擎。WASM 模組已打包在套件內部 — 不需要額外的設定。

## Rust

請先下載 `rust-crate` artifact，解開其中的 `.crate` 檔，再以本地 path dependency 使用。

```bash
mkdir -p vendor/ttoon-core
tar -xzf ./ttoon-core-0.1.0.crate -C vendor/ttoon-core
```

接著在 `Cargo.toml` 中加入：

```toml
[dependencies]
ttoon-core = { path = "vendor/ttoon-core" }
```

`ttoon-core` crate 預設已包含 Apache Arrow 支援。

## 驗證安裝

### Python

```python
import ttoon
print(ttoon.dumps({"hello": "world"}))
# hello: "world"
```

### JavaScript

```ts
import { stringify } from '@ttoon/shared';
console.log(stringify({ hello: 'world' }));
// hello: "world"
```

### Rust

```rust
use ttoon_core::{from_ttoon, to_ttoon};
let node = from_ttoon("hello: \"world\"").unwrap();
let text = to_ttoon(&node, None).unwrap();
assert_eq!(text, "hello: \"world\"\n");
```
