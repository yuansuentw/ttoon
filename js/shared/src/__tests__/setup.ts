import { readFile } from 'node:fs/promises';
import { initWasm } from '../index.js';

const wasmBytes = await readFile(new URL('../../../../rust/crates/wasm-bridge/pkg/ttoon_wasm_bridge_bg.wasm', import.meta.url));

await initWasm(wasmBytes);
