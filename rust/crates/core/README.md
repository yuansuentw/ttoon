# ttoon-core

`ttoon-core` is the core Rust library for TTOON.

## Features

- `T-TOON` and `T-JSON` parsing and serialization
- typed value parsing rules
- Arrow schema inference and text conversion

## Installation

```bash
cargo add ttoon-core
```

## Quick Example

```rust
use ttoon_core::{from_ttoon, to_tjson};

let node = from_ttoon("name: \"Alice\"\nscore: 95")?;
let text = to_tjson(&node, None)?;
# Ok::<(), ttoon_core::Error>(())
```

## Main APIs

- `from_ttoon()`
- `from_ttoon_with_mode()`
- `to_ttoon()`
- `to_tjson()`
- `read_arrow()`
- `arrow_to_ttoon()`
- `arrow_to_tjson()`
- `detect_format()`

See `docs/public/en/` for guides and API reference material.
