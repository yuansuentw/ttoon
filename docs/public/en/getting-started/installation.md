---
title: Installation
sidebar_position: 1
sidebar_label: Installation
description: Install TTOON packages for Python, JavaScript/TypeScript, and Rust.
---

# Installation

TTOON `0.1.x` packages are published to PyPI, npm, and crates.io. The official documentation site is [ttoon.dev](https://ttoon.dev/).

import Tabs from '@theme/Tabs';
import TabItem from '@theme/TabItem';

<Tabs>
<TabItem value="python" label="Python">

```bash
pip install ttoon
```

If you need to install from a source distribution manually:

```bash
pip install ./ttoon-0.1.0.tar.gz
```

`pyarrow` and `polars` are already declared as package dependencies. No extra step is needed for the normal wheel install.

The current Python package depends on `pyarrow>=23.0.0` and `polars>=1.37.1`.

If you are working from a stripped-down environment, install them explicitly:

```bash
pip install pyarrow polars
```

Requires Python 3.11+. Installing from wheel does not require a Rust toolchain; installing from sdist does.

</TabItem>
<TabItem value="js" label="JavaScript / TypeScript">

```bash
npm install @ttoon/shared
```

For Arrow table operations, add the optional peer dependency:

```bash
npm install @ttoon/shared apache-arrow
```

For custom decimal codecs, install the library your codec uses. Common choices are `decimal.js` and `big.js`:

```bash
npm install @ttoon/shared decimal.js
npm install @ttoon/shared big.js
```

> **Note**: `@ttoon/node` and `@ttoon/web` are environment-specific re-exports of `@ttoon/shared`. Install `@ttoon/shared` directly unless you want explicit environment-specific imports.

```bash
npm install @ttoon/node
npm install @ttoon/web
```

The JS SDK uses a WASM bridge to invoke the Rust core engine. The WASM module is bundled inside the package — no additional setup required.

</TabItem>
<TabItem value="rust" label="Rust">

```bash
cargo add ttoon-core
```

The `ttoon-core` crate includes Apache Arrow support by default.

</TabItem>
</Tabs>

## Official SDKs

All SDKs share the same Rust core, ensuring consistent parsing and serialization behavior. The JS packages are split by runtime target, but `@ttoon/node` and `@ttoon/web` are thin re-exports of `@ttoon/shared`.

| Language | Package | Architecture |
| :--- | :--- | :--- |
| Python | `ttoon` | Rust core via PyO3 |
| JS / TS | `@ttoon/shared` | Rust core via WASM |
| Node.js | `@ttoon/node` | Re-exports shared |
| Web | `@ttoon/web` | Re-exports shared |
| Rust | `ttoon-core` | Core engine |

## Verify Installation

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
