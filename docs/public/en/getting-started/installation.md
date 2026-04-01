---
title: Installation
sidebar_position: 1
sidebar_label: Installation
description: Install TTOON packages for Python, JavaScript/TypeScript, and Rust.
---

# Installation

TTOON `0.1.x` packages are **not published to PyPI, npm, or crates.io**. Public releases provide local-install package artifacts only. Download the matching artifact from GitHub Releases or GitHub Actions, then install from the local file.

## Python

Download the `python-wheel-*` or `python-sdist` artifact first.

```bash
pip install ./ttoon-0.1.0-*.whl
```

If you only have the source distribution:

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

## JavaScript / TypeScript

Download the `js-packages` artifact first. It contains `ttoon-shared-0.1.0.tgz`, `ttoon-node-0.1.0.tgz`, and `ttoon-web-0.1.0.tgz`.

```bash
npm install ./ttoon-shared-0.1.0.tgz
```

For Arrow table operations, add the optional peer dependency:

```bash
npm install ./ttoon-shared-0.1.0.tgz apache-arrow
```

For custom decimal codecs, install the library your codec uses. Common choices are `decimal.js` and `big.js`:

```bash
npm install ./ttoon-shared-0.1.0.tgz decimal.js
npm install ./ttoon-shared-0.1.0.tgz big.js
```

> **Note**: `@ttoon/node` and `@ttoon/web` are environment-specific re-exports of `@ttoon/shared`. If you install them, install the matching `ttoon-shared-0.1.0.tgz` together.

```bash
npm install ./ttoon-shared-0.1.0.tgz ./ttoon-node-0.1.0.tgz
npm install ./ttoon-shared-0.1.0.tgz ./ttoon-web-0.1.0.tgz
```

The JS SDK uses a WASM bridge to invoke the Rust core engine. The WASM module is bundled inside the package — no additional setup required.

## Rust

Download the `rust-crate` artifact first, unpack the `.crate`, then consume it as a local path dependency.

```bash
mkdir -p vendor/ttoon-core
tar -xzf ./ttoon-core-0.1.0.crate -C vendor/ttoon-core
```

Then add this to `Cargo.toml`:

```toml
[dependencies]
ttoon-core = { path = "vendor/ttoon-core" }
```

The `ttoon-core` crate includes Apache Arrow support by default.

## Verify Installation

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
