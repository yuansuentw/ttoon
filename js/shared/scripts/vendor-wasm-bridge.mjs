import { copyFile, mkdir, readFile, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const sharedDir = path.resolve(scriptDir, '..');
const repoRoot = path.resolve(sharedDir, '..', '..');
const wasmPkgDir = path.join(repoRoot, 'rust', 'crates', 'wasm-bridge', 'pkg');
const distDir = path.join(sharedDir, 'dist');

const wasmFiles = [
  'ttoon_wasm_bridge.js',
  'ttoon_wasm_bridge.d.ts',
  'ttoon_wasm_bridge_bg.wasm',
  'ttoon_wasm_bridge_bg.wasm.d.ts',
];

const distFilesToRewrite = [
  'index.js',
  'index.d.ts',
];

const replacements = [
  ['"ttoon-wasm-bridge"', '"./ttoon_wasm_bridge.js"'],
  ["'ttoon-wasm-bridge'", "'./ttoon_wasm_bridge.js'"],
];

await mkdir(distDir, { recursive: true });

for (const file of wasmFiles) {
  await copyFile(path.join(wasmPkgDir, file), path.join(distDir, file));
}

for (const file of distFilesToRewrite) {
  const targetPath = path.join(distDir, file);
  let content;
  try {
    content = await readFile(targetPath, 'utf8');
  } catch (error) {
    if (error && typeof error === 'object' && 'code' in error && error.code === 'ENOENT') {
      continue;
    }
    throw error;
  }
  for (const [from, to] of replacements) {
    content = content.split(from).join(to);
  }
  await writeFile(targetPath, content);
}

process.stdout.write('[js] vendored wasm bridge into dist\n');
