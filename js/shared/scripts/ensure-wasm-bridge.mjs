import { existsSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { spawnSync } from 'node:child_process';

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(scriptDir, '../../..');
const wasmBridgeDir = path.join(repoRoot, 'rust', 'crates', 'wasm-bridge');
const wasmBridgePkgDir = path.join(wasmBridgeDir, 'pkg');
const wasmBridgeFiles = [
  'package.json',
  'ttoon_wasm_bridge.js',
  'ttoon_wasm_bridge_bg.wasm',
];

if (wasmBridgeFiles.every((file) => existsSync(path.join(wasmBridgePkgDir, file)))) {
  process.stdout.write('[js] wasm bridge pkg already present\n');
  process.exit(0);
}

process.stdout.write('[js] wasm bridge pkg missing, building with wasm-pack\n');

const result = spawnSync('wasm-pack', ['build', '--target', 'web', '--out-dir', 'pkg'], {
  cwd: wasmBridgeDir,
  stdio: 'inherit',
});

if (result.error) {
  if (result.error.code === 'ENOENT' || result.error.code === 'EACCES') {
    process.stderr.write('[js] error: wasm-pack is unavailable in this environment. Install wasm-pack or run JS CI on GitHub Actions.\n');
    process.exit(1);
  }
  throw result.error;
}

if (result.status !== 0) {
  process.exit(result.status ?? 1);
}

if (!wasmBridgeFiles.every((file) => existsSync(path.join(wasmBridgePkgDir, file)))) {
  process.stderr.write('[js] error: wasm-pack completed but required pkg artifacts are still missing.\n');
  process.exit(1);
}

process.stdout.write('[js] wasm bridge pkg ready\n');
