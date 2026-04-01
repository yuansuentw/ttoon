import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import process from 'node:process';
import { spawnSync } from 'node:child_process';
import { performance } from 'node:perf_hooks';
import { fileURLToPath, pathToFileURL } from 'node:url';

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../../..');
const BENCHMARK_JS_ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const BENCHMARK_MANIFEST_PATH = path.join(ROOT, 'benchmarks/manifests/benchmark_release.sh');
const DATASET_MANIFEST_PATH = path.join(ROOT, 'benchmarks/manifests/datasets.sh');
const BENCHMARK_NODE_MODULES = path.join(BENCHMARK_JS_ROOT, 'node_modules');
const WASM_BRIDGE_ROOT = path.join(ROOT, 'rust/crates/wasm-bridge');
const WASM_BRIDGE_PKG_ROOT = path.join(WASM_BRIDGE_ROOT, 'pkg');
const INSTALLED_SHARED_ROOT = path.join(BENCHMARK_NODE_MODULES, '@ttoon', 'shared');
const TSX_ESM_API_PATH = path.join(
  BENCHMARK_NODE_MODULES,
  'tsx/dist/esm/api/index.mjs',
);
const SHARED_INDEX_TS_PATH = path.join(INSTALLED_SHARED_ROOT, 'src/index.ts');
const WASM_BRIDGE_PKG_PACKAGE_JSON = path.join(WASM_BRIDGE_PKG_ROOT, 'package.json');
const APACHE_ARROW_PACKAGE_PATH = path.join(
  BENCHMARK_NODE_MODULES,
  'apache-arrow/package.json',
);
const DEFAULT_WARMUPS = 2;
const DEFAULT_ITERATIONS = 20;
const RUNNER_SCHEMA_VERSION = 1;
const CASE_MATRIX = {
  'js-basic': {
    structure: [
      'json_serialize',
      'json_deserialize',
      'tjson_serialize',
      'tjson_deserialize',
      'ttoon_serialize',
      'ttoon_deserialize',
      'toon_serialize',
      'toon_deserialize',
    ],
    tabular: [
      'arrow_tjson_serialize',
      'arrow_tjson_deserialize',
      'arrow_ttoon_serialize',
      'arrow_ttoon_deserialize',
    ],
  },
  extended: {
    structure: ['ttoon_serialize', 'ttoon_deserialize'],
    tabular: [
      'arrow_ttoon_serialize',
      'arrow_ttoon_deserialize',
      'arrow_tjson_serialize',
      'arrow_tjson_deserialize',
    ],
  },
};

function parseArgs(argv) {
  const options = {
    datasetRoot: path.join(ROOT, 'benchmarks/datasets/prepared'),
    variant: null,
    size: null,
    shape: null,
    case: null,
    warmups: DEFAULT_WARMUPS,
    iterations: DEFAULT_ITERATIONS,
    benchmarkRelease: null,
    datasetRelease: null,
    listCases: false,
    traceMemory: false,
  };

  for (let index = 0; index < argv.length; index += 1) {
    const token = argv[index];
    if (token === '--list-cases') {
      options.listCases = true;
      continue;
    }
    if (token === '--trace-memory') {
      options.traceMemory = true;
      continue;
    }

    const value = argv[index + 1];
    if (value === undefined) {
      throw new Error(`missing parameter value: ${token}`);
    }

    switch (token) {
      case '--dataset-root':
        options.datasetRoot = path.resolve(value);
        break;
      case '--variant':
        options.variant = value;
        break;
      case '--size':
        options.size = value;
        break;
      case '--shape':
        options.shape = value;
        break;
      case '--case':
        options.case = value;
        break;
      case '--warmups':
        options.warmups = Number.parseInt(value, 10);
        break;
      case '--iterations':
        options.iterations = Number.parseInt(value, 10);
        break;
      case '--benchmark-release':
        options.benchmarkRelease = value;
        break;
      case '--dataset-release':
        options.datasetRelease = Number.parseInt(value, 10);
        break;
      default:
        throw new Error(`unknown argument: ${token}`);
    }
    index += 1;
  }

  return options;
}

function listCaseEntries(variantFilter, shapeFilter) {
  const rows = [];
  for (const [variant, shapeMap] of Object.entries(CASE_MATRIX)) {
    if (variantFilter && variantFilter !== variant) {
      continue;
    }
    for (const [shape, cases] of Object.entries(shapeMap)) {
      if (shapeFilter && shapeFilter !== shape) {
        continue;
      }
      for (const caseName of cases) {
        rows.push({
          language: 'js',
          variant,
          shape,
          case: caseName,
        });
      }
    }
  }
  return rows;
}

function readReleaseMetadata(options) {
  const benchmarkManifest = parseShellScalars(BENCHMARK_MANIFEST_PATH);
  const datasetManifest = parseShellScalars(DATASET_MANIFEST_PATH);
  const benchmarkRelease = requiredScalar(
    benchmarkManifest,
    'BENCHMARK_RELEASE',
    BENCHMARK_MANIFEST_PATH,
  );
  const benchmarkDatasetRelease = requiredScalar(
    benchmarkManifest,
    'BENCHMARK_DATASET_RELEASE',
    BENCHMARK_MANIFEST_PATH,
  );
  const datasetReleaseText = requiredScalar(
    datasetManifest,
    'DATASET_RELEASE',
    DATASET_MANIFEST_PATH,
  );
  const datasetRelease = parseDatasetRelease(datasetReleaseText);
  const major = parseBenchmarkReleaseMajor(benchmarkRelease);
  if (String(major) !== benchmarkDatasetRelease) {
    throw new Error(
      `BENCHMARK_RELEASE does not match BENCHMARK_DATASET_RELEASE: ${benchmarkRelease} vs ${benchmarkDatasetRelease}`,
    );
  }
  if (benchmarkDatasetRelease !== String(datasetRelease)) {
    throw new Error(
      `BENCHMARK_DATASET_RELEASE does not match DATASET_RELEASE: ${benchmarkDatasetRelease} vs ${datasetRelease}`,
    );
  }
  if (options.benchmarkRelease !== null && options.benchmarkRelease !== benchmarkRelease) {
    throw new Error(
      `CLI benchmark_release does not match authoritative manifest: ${options.benchmarkRelease} vs ${benchmarkRelease}`,
    );
  }
  if (options.datasetRelease !== null && !Number.isInteger(options.datasetRelease)) {
    throw new Error(`invalid CLI dataset_release: ${options.datasetRelease}`);
  }
  if (options.datasetRelease !== null && options.datasetRelease !== datasetRelease) {
    throw new Error(
      `CLI dataset_release does not match authoritative manifest: ${options.datasetRelease} vs ${datasetRelease}`,
    );
  }
  return {
    benchmarkRelease,
    datasetRelease,
  };
}

function parseShellScalars(filePath) {
  const assignments = {};
  const lines = fs.readFileSync(filePath, 'utf8').split(/\r?\n/u);
  for (const rawLine of lines) {
    const line = rawLine.trim();
    if (!line || line.startsWith('#') || line.startsWith('declare -A ')) {
      continue;
    }
    const match = /^([A-Z0-9_]+)=(.*)$/u.exec(line);
    if (!match) {
      continue;
    }
    const [, key, rawValue] = match;
    const value = rawValue.trim();
    if (value.startsWith('(')) {
      continue;
    }
    assignments[key] = parseShellScalar(value);
  }
  return assignments;
}

function parseShellScalar(rawValue) {
  if (
    (rawValue.startsWith('"') && rawValue.endsWith('"'))
    || (rawValue.startsWith("'") && rawValue.endsWith("'"))
  ) {
    return rawValue.slice(1, -1);
  }
  throw new Error(`unsupported shell scalar: ${rawValue}`);
}

function requiredScalar(assignments, key, filePath) {
  const value = assignments[key];
  if (typeof value !== 'string') {
    throw new Error(`${path.relative(ROOT, filePath)} missing required field: ${key}`);
  }
  return value;
}

function parseBenchmarkReleaseMajor(benchmarkRelease) {
  const match = /^(\d+)\.(\d+)$/u.exec(benchmarkRelease);
  if (!match) {
    throw new Error(`invalid BENCHMARK_RELEASE: ${benchmarkRelease}`);
  }
  return Number.parseInt(match[1], 10);
}

function parseDatasetRelease(datasetRelease) {
  if (!/^\d+$/u.test(datasetRelease)) {
    throw new Error(`invalid DATASET_RELEASE: ${datasetRelease}`);
  }
  return Number.parseInt(datasetRelease, 10);
}

function discoverDatasets(datasetRoot, variantFilter, sizeFilter) {
  if (!fs.existsSync(datasetRoot)) {
    return [];
  }

  const rows = [];
  for (const variant of fs.readdirSync(datasetRoot)) {
    if (variantFilter && variantFilter !== variant) {
      continue;
    }
    const variantDir = path.join(datasetRoot, variant);
    if (!fs.statSync(variantDir).isDirectory()) {
      continue;
    }
    for (const size of fs.readdirSync(variantDir)) {
      if (sizeFilter && sizeFilter !== size) {
        continue;
      }
      const bundleRoot = path.join(variantDir, size);
      const metaPath = path.join(bundleRoot, 'meta.json');
      if (!fs.existsSync(metaPath)) {
        continue;
      }
      const meta = JSON.parse(fs.readFileSync(metaPath, 'utf8'));
      rows.push({
        variant,
        size,
        rowCount: meta.row_count ?? null,
        metaPath,
        bundleRoot,
        fileFor(shape, filename) {
          return path.join(bundleRoot, shape, filename);
        },
      });
    }
  }
  return rows;
}

function environmentInfo() {
  return {
    os: process.platform,
    os_release: os.release(),
    platform: os.version?.() ?? process.platform,
    architecture: process.arch,
    cpu_model: os.cpus()[0]?.model ?? 'unknown',
    node_version: process.version,
    python_version: null,
    rust_version: readCommandVersion(['rustc', '--version']),
  };
}

function readCommandVersion(command) {
  try {
    const completed = spawnSync(command[0], command.slice(1), {
      encoding: 'utf8',
    });
    if (completed.status !== 0) {
      return null;
    }
    return completed.stdout.trim() || null;
  } catch {
    return null;
  }
}

function relativeToRoot(targetPath) {
  return path.relative(ROOT, targetPath);
}

function addIssue(issueSet, message) {
  issueSet.add(message);
}

function hasLocalJsRuntime() {
  return [
    TSX_ESM_API_PATH,
    APACHE_ARROW_PACKAGE_PATH,
  ].every((filePath) => fs.existsSync(filePath));
}

function installLocalJsRuntime() {
  const completed = spawnSync(
    'npm',
    ['install'],
    {
      cwd: BENCHMARK_JS_ROOT,
      encoding: 'utf8',
      env: {
        ...process.env,
        npm_config_audit: 'false',
        npm_config_fund: 'false',
      },
    },
  );

  if (completed.status !== 0) {
    const message = completed.stderr.trim() || completed.stdout.trim() || 'npm install failed';
    throw new Error(`benchmarks/js auto-install of local JS dependencies failed: ${message}`);
  }
}

function ensureLocalJsRuntime() {
  if (hasLocalJsRuntime()) {
    return;
  }

  installLocalJsRuntime();

  if (!hasLocalJsRuntime()) {
    throw new Error('benchmarks/js local dependencies installed, but required files for JS benchmark are still missing');
  }
}

async function loadSharedModule() {
  ensureSharedPackage();
  const { tsImport } = await import(pathToFileURL(TSX_ESM_API_PATH).href);
  return tsImport(SHARED_INDEX_TS_PATH, import.meta.url);
}

function hasWasmBridgePkg() {
  return fs.existsSync(WASM_BRIDGE_PKG_PACKAGE_JSON);
}

function buildWasmBridgePkg() {
  const completed = spawnSync(
    'wasm-pack',
    ['build', '--target', 'nodejs', '--release', '--out-dir', 'pkg'],
    {
      cwd: WASM_BRIDGE_ROOT,
      encoding: 'utf8',
      env: {
        ...process.env,
      },
    },
  );

  if (completed.status !== 0) {
    const message = completed.stderr.trim() || completed.stdout.trim() || 'wasm-pack build failed';
    throw new Error(`auto-build of ttoon-wasm-bridge pkg failed: ${message}`);
  }
}

function ensureWasmBridgePkg() {
  if (hasWasmBridgePkg()) {
    return;
  }
  if (!readCommandVersion(['wasm-pack', '--version'])) {
    throw new Error(
      'missing rust/crates/wasm-bridge/pkg and wasm-pack is not installed; cannot set up @ttoon/shared benchmark target',
    );
  }
  buildWasmBridgePkg();
  if (!hasWasmBridgePkg()) {
    throw new Error('wasm-pack build completed, but rust/crates/wasm-bridge/pkg/package.json is still missing');
  }
}

function hasInstalledSharedPackage() {
  return [
    path.join(INSTALLED_SHARED_ROOT, 'package.json'),
    SHARED_INDEX_TS_PATH,
    path.join(BENCHMARK_NODE_MODULES, 'ttoon-wasm-bridge', 'package.json'),
  ].every((filePath) => fs.existsSync(filePath));
}

function installSharedPackage() {
  ensureWasmBridgePkg();
  const completed = spawnSync(
    'npm',
    ['install', '--no-save', '--no-package-lock', '../../js/shared'],
    {
      cwd: BENCHMARK_JS_ROOT,
      encoding: 'utf8',
      env: {
        ...process.env,
        npm_config_audit: 'false',
        npm_config_fund: 'false',
        npm_config_package_lock: 'false',
      },
    },
  );

  if (completed.status !== 0) {
    const message = completed.stderr.trim() || completed.stdout.trim() || 'npm install failed';
    throw new Error(`benchmarks/js auto-install of @ttoon/shared failed: ${message}`);
  }
}

function ensureSharedPackage() {
  if (hasInstalledSharedPackage()) {
    return;
  }
  installSharedPackage();
  if (!hasInstalledSharedPackage()) {
    throw new Error('@ttoon/shared installed, but src/index.ts not found in benchmarks/js/node_modules');
  }
}

function hasInstalledToonPackage() {
  return fs.existsSync(path.join(BENCHMARK_NODE_MODULES, '@toon-format', 'toon', 'package.json'));
}

function installToonPackage() {
  const completed = spawnSync(
    'npm',
    ['install', '--no-save', '--no-package-lock', '@toon-format/toon'],
    {
      cwd: BENCHMARK_JS_ROOT,
      encoding: 'utf8',
      env: {
        ...process.env,
        npm_config_audit: 'false',
        npm_config_fund: 'false',
        npm_config_package_lock: 'false',
      },
    },
  );

  if (completed.status !== 0) {
    const message = completed.stderr.trim() || completed.stdout.trim() || 'npm install failed';
    throw new Error(`benchmarks/js auto-install of @toon-format/toon failed: ${message}`);
  }
}

function ensureToonPackage() {
  if (hasInstalledToonPackage()) {
    return;
  }
  installToonPackage();
  if (!hasInstalledToonPackage()) {
    throw new Error('@toon-format/toon installed, but package not found in benchmarks/js/node_modules');
  }
}

async function loadToonModule() {
  ensureToonPackage();
  return import('@toon-format/toon');
}

async function loadApacheArrow() {
  return import('apache-arrow');
}

function readText(filePath) {
  return fs.readFileSync(filePath, 'utf8');
}

function readJson(filePath) {
  return JSON.parse(readText(filePath));
}

function isTypedWrapper(value) {
  return (
    value
    && typeof value === 'object'
    && !Array.isArray(value)
    && Object.keys(value).length === 2
    && Object.hasOwn(value, '$kind')
    && Object.hasOwn(value, 'value')
  );
}

function hydrateObjectSource(value, shared) {
  if (Array.isArray(value)) {
    return value.map((item) => hydrateObjectSource(item, shared));
  }
  if (value && typeof value === 'object') {
    if (isTypedWrapper(value)) {
      const raw = String(value.value);
      switch (value.$kind) {
        case 'decimal':
          return shared.toon.decimal(raw);
        case 'uuid':
          return shared.toon.uuid(raw);
        case 'date':
          return shared.toon.date(raw);
        case 'time':
          return shared.toon.time(raw);
        case 'datetime':
          return shared.toon.datetime(raw);
        default:
          throw new Error(`unknown typed wrapper kind: ${value.$kind}`);
      }
    }

    const hydrated = {};
    for (const [key, item] of Object.entries(value)) {
      hydrated[key] = hydrateObjectSource(item, shared);
    }
    return hydrated;
  }
  return value;
}

function summarizeSamples(samplesMs) {
  if (samplesMs.length === 0) {
    throw new Error('samplesMs must not be empty');
  }

  const sorted = [...samplesMs].sort((left, right) => left - right);
  const mean = samplesMs.reduce((sum, value) => sum + value, 0) / samplesMs.length;
  const median = sorted.length % 2 === 0
    ? (sorted[(sorted.length / 2) - 1] + sorted[sorted.length / 2]) / 2
    : sorted[Math.floor(sorted.length / 2)];
  const variance = samplesMs.length > 1
    ? samplesMs.reduce((sum, value) => sum + ((value - mean) ** 2), 0) / (samplesMs.length - 1)
    : 0;

  return {
    mean_ms: mean,
    median_ms: median,
    min_ms: Math.min(...samplesMs),
    max_ms: Math.max(...samplesMs),
    stdev_ms: Math.sqrt(variance),
  };
}

async function measureSync(func, warmups, iterations, traceMemory = false) {
  for (let index = 0; index < warmups; index += 1) {
    func();
  }

  const samplesMs = [];
  const memoryTraceKb = [];
  for (let index = 0; index < iterations; index += 1) {
    const startedAt = performance.now();
    func();
    samplesMs.push(performance.now() - startedAt);
    if (traceMemory) {
      memoryTraceKb.push(Math.round(process.memoryUsage().rss / 1024));
    }
  }
  return [samplesMs, summarizeSamples(samplesMs), traceMemory ? memoryTraceKb : null];
}

async function measureAsync(func, warmups, iterations, traceMemory = false) {
  for (let index = 0; index < warmups; index += 1) {
    await func();
  }

  const samplesMs = [];
  const memoryTraceKb = [];
  for (let index = 0; index < iterations; index += 1) {
    const startedAt = performance.now();
    await func();
    samplesMs.push(performance.now() - startedAt);
    if (traceMemory) {
      memoryTraceKb.push(Math.round(process.memoryUsage().rss / 1024));
    }
  }
  return [samplesMs, summarizeSamples(samplesMs), traceMemory ? memoryTraceKb : null];
}

async function loadCached(cache, key, loader) {
  if (!cache.has(key)) {
    cache.set(key, await loader());
  }
  return cache.get(key);
}

async function prepareCase(bundle, shape, caseName, context, cache, issueSet) {
  const { shared, apacheArrow, toonPackage } = context;

  if (caseName.startsWith('json_')) {
    if (shape !== 'structure') {
      return null;
    }
    const sourcePath = bundle.fileFor('structure', 'source.json');
    if (!fs.existsSync(sourcePath)) {
      addIssue(issueSet, `${relativeToRoot(bundle.metaPath)}: missing structure/source.json, skipping ${caseName}`);
      return null;
    }

    if (caseName === 'json_serialize') {
      const value = await loadCached(
        cache,
        `structure-source:${sourcePath}`,
        async () => hydrateObjectSource(readJson(sourcePath), shared),
      );
      return {
      kind: 'sync',
      inputHint: relativeToRoot(sourcePath),
      execute: () => JSON.stringify(value),
      };
    }

    const text = await loadCached(
      cache,
      `text:${sourcePath}`,
      async () => readText(sourcePath),
    );
    return {
      kind: 'sync',
      inputHint: relativeToRoot(sourcePath),
      execute: () => JSON.parse(text),
    };
  }

  if (shape === 'structure') {
    const sourcePath = bundle.fileFor('structure', 'source.json');
    const tjsonPath = bundle.fileFor('structure', 'tjson.txt');
    const ttoonPath = bundle.fileFor('structure', 'ttoon.txt');
    const toonPath = bundle.fileFor('structure', 'toon.txt');

    if (caseName === 'tjson_serialize') {
      if (!fs.existsSync(sourcePath)) {
        addIssue(issueSet, `${relativeToRoot(bundle.metaPath)}: missing structure/source.json, skipping ${caseName}`);
        return null;
      }
      const value = await loadCached(
        cache,
        `object-source:${sourcePath}`,
        async () => hydrateObjectSource(readJson(sourcePath), shared),
      );
      return {
        kind: 'sync',
        inputHint: relativeToRoot(sourcePath),
        execute: () => shared.toTjson(value),
      };
    }

    if (caseName === 'tjson_deserialize') {
      if (!fs.existsSync(tjsonPath)) {
        addIssue(issueSet, `${relativeToRoot(bundle.metaPath)}: missing structure/tjson.txt, skipping ${caseName}`);
        return null;
      }
      const text = await loadCached(cache, `text:${tjsonPath}`, async () => readText(tjsonPath));
      return {
        kind: 'sync',
        inputHint: relativeToRoot(tjsonPath),
        execute: () => shared.parse(text),
      };
    }

    if (caseName === 'ttoon_serialize') {
      if (!fs.existsSync(sourcePath)) {
        addIssue(issueSet, `${relativeToRoot(bundle.metaPath)}: missing structure/source.json, skipping ${caseName}`);
        return null;
      }
      const value = await loadCached(
        cache,
        `structure-source:${sourcePath}`,
        async () => hydrateObjectSource(readJson(sourcePath), shared),
      );
      return {
        kind: 'sync',
        inputHint: relativeToRoot(sourcePath),
        execute: () => shared.stringify(value),
      };
    }

    if (caseName === 'ttoon_deserialize') {
      if (!fs.existsSync(ttoonPath)) {
        addIssue(issueSet, `${relativeToRoot(bundle.metaPath)}: missing structure/ttoon.txt, skipping ${caseName}`);
        return null;
      }
      const text = await loadCached(cache, `text:${ttoonPath}`, async () => readText(ttoonPath));
      return {
        kind: 'sync',
        inputHint: relativeToRoot(ttoonPath),
        execute: () => shared.parse(text),
      };
    }

    if (caseName === 'toon_serialize') {
      if (!toonPackage) {
        addIssue(issueSet, 'missing @toon-format/toon, skipping toon_serialize');
        return null;
      }
      if (typeof toonPackage.stringify !== 'function') {
        addIssue(issueSet, '@toon-format/toon does not provide stringify(), skipping toon_serialize');
        return null;
      }
      if (!fs.existsSync(sourcePath)) {
        addIssue(issueSet, `${relativeToRoot(bundle.metaPath)}: missing structure/source.json, skipping ${caseName}`);
        return null;
      }
      const value = await loadCached(
        cache,
        `structure-source-json-basic:${sourcePath}`,
        async () => readJson(sourcePath),
      );
      return {
        kind: 'sync',
        inputHint: relativeToRoot(sourcePath),
        execute: () => toonPackage.stringify(value),
      };
    }

    if (caseName === 'toon_deserialize') {
      if (!toonPackage) {
        addIssue(issueSet, 'missing @toon-format/toon, skipping toon_deserialize');
        return null;
      }
      if (typeof toonPackage.parse !== 'function') {
        addIssue(issueSet, '@toon-format/toon does not provide parse(), skipping toon_deserialize');
        return null;
      }
      if (!fs.existsSync(toonPath)) {
        addIssue(issueSet, `${relativeToRoot(bundle.metaPath)}: missing structure/toon.txt, skipping ${caseName}`);
        return null;
      }
      const text = await loadCached(cache, `text:${toonPath}`, async () => readText(toonPath));
      return {
        kind: 'sync',
        inputHint: relativeToRoot(toonPath),
        execute: () => toonPackage.parse(text),
      };
    }

    return null;
  }

  const sourceArrowPath = bundle.fileFor('tabular', 'source.arrow');
  const tjsonPath = bundle.fileFor('tabular', 'tjson.txt');
  const ttoonPath = bundle.fileFor('tabular', 'ttoon.txt');
  const arrowLoader = (ipcBytes) => apacheArrow.tableFromIPC(ipcBytes);

  if (caseName === 'arrow_tjson_serialize') {
    if (!fs.existsSync(sourceArrowPath)) {
      addIssue(issueSet, `${relativeToRoot(bundle.metaPath)}: missing tabular/source.arrow, skipping ${caseName}`);
      return null;
    }
    const table = await loadCached(
      cache,
      `arrow:${sourceArrowPath}`,
      async () => arrowLoader(fs.readFileSync(sourceArrowPath)),
    );
    return {
      kind: 'async',
      inputHint: relativeToRoot(sourceArrowPath),
      execute: () => shared.stringifyArrowTjson(table),
    };
  }

  if (caseName === 'arrow_tjson_deserialize') {
    if (!fs.existsSync(tjsonPath)) {
      addIssue(issueSet, `${relativeToRoot(bundle.metaPath)}: missing tabular/tjson.txt, skipping ${caseName}`);
      return null;
    }
    const text = await loadCached(cache, `text:${tjsonPath}`, async () => readText(tjsonPath));
    return {
      kind: 'async',
      inputHint: relativeToRoot(tjsonPath),
      execute: () => shared.readArrow(text),
    };
  }

  if (caseName === 'arrow_ttoon_serialize') {
    if (!fs.existsSync(sourceArrowPath)) {
      addIssue(issueSet, `${relativeToRoot(bundle.metaPath)}: missing tabular/source.arrow, skipping ${caseName}`);
      return null;
    }
    const table = await loadCached(
      cache,
      `arrow:${sourceArrowPath}`,
      async () => arrowLoader(fs.readFileSync(sourceArrowPath)),
    );
    return {
      kind: 'async',
      inputHint: relativeToRoot(sourceArrowPath),
      execute: () => shared.stringifyArrow(table),
    };
  }

  if (caseName === 'arrow_ttoon_deserialize') {
    if (!fs.existsSync(ttoonPath)) {
      addIssue(issueSet, `${relativeToRoot(bundle.metaPath)}: missing tabular/ttoon.txt, skipping ${caseName}`);
      return null;
    }
    const text = await loadCached(cache, `text:${ttoonPath}`, async () => readText(ttoonPath));
    return {
      kind: 'async',
      inputHint: relativeToRoot(ttoonPath),
      execute: () => shared.readArrow(text),
    };
  }

  return null;
}

async function runBenchmarks(options) {
  const releaseMetadata = readReleaseMetadata(options);
  const issueSet = new Set();
  const results = [];
  const datasets = discoverDatasets(options.datasetRoot, options.variant, options.size);
  const cache = new Map();

  let shared = null;
  let apacheArrow = null;
  let toonPackage = null;
  let jsRuntimeReady = false;

  try {
    ensureLocalJsRuntime();
    jsRuntimeReady = true;
  } catch (error) {
    addIssue(issueSet, error instanceof Error ? error.message : String(error));
  }

  if (jsRuntimeReady) {
    try {
      shared = await loadSharedModule();
    } catch (error) {
      addIssue(issueSet, `failed to install or load JS shared module: ${error instanceof Error ? error.message : String(error)}`);
    }

    try {
      apacheArrow = await loadApacheArrow();
    } catch (error) {
      addIssue(issueSet, `failed to load apache-arrow: ${error instanceof Error ? error.message : String(error)}`);
    }
    if (needsToonPackage(options, datasets)) {
      try {
        toonPackage = await loadToonModule();
      } catch (error) {
        addIssue(
          issueSet,
          `failed to install or load @toon-format/toon benchmark comparison package: ${error instanceof Error ? error.message : String(error)}`,
        );
      }
    }
  }

  if (!shared || !apacheArrow) {
    return {
      schema_version: RUNNER_SCHEMA_VERSION,
      benchmark_release: releaseMetadata.benchmarkRelease,
      dataset_release: releaseMetadata.datasetRelease,
      language: 'js',
      generated_at: new Date().toISOString(),
      filters: {
        variant: options.variant,
        size: options.size,
        shape: options.shape,
        case: options.case,
        warmups: options.warmups,
        iterations: options.iterations,
      },
      environment: environmentInfo(),
      dataset_count: datasets.length,
      discovered_datasets: datasets.map((dataset) => ({
        variant: dataset.variant,
        size: dataset.size,
        row_count: dataset.rowCount,
        meta_path: relativeToRoot(dataset.metaPath),
      })),
      results: [],
      issues: [...issueSet],
    };
  }

  const context = { shared, apacheArrow, toonPackage };

  for (const bundle of datasets) {
    const shapeMap = CASE_MATRIX[bundle.variant] ?? {};
    for (const [shape, caseNames] of Object.entries(shapeMap)) {
      if (options.shape && options.shape !== shape) {
        continue;
      }
      for (const caseName of caseNames) {
        if (options.case && options.case !== caseName) {
          continue;
        }

        try {
          const prepared = await prepareCase(bundle, shape, caseName, context, cache, issueSet);
          if (!prepared) {
            continue;
          }

          const [samplesMs, stats, memoryTraceKb] = prepared.kind === 'async'
            ? await measureAsync(prepared.execute, options.warmups, options.iterations, options.traceMemory)
            : await measureSync(prepared.execute, options.warmups, options.iterations, options.traceMemory);

          const result = {
            variant: bundle.variant,
            shape,
            size: bundle.size,
            row_count: bundle.rowCount,
            case: caseName,
            warmups: options.warmups,
            iterations: options.iterations,
            stats,
            samples_ms: samplesMs,
            input_hint: prepared.inputHint,
          };
          if (memoryTraceKb) {
            result.memory_trace_kb = memoryTraceKb;
          }
          results.push(result);
        } catch (error) {
          addIssue(
            issueSet,
            `${relativeToRoot(bundle.metaPath)}: ${caseName} failed: ${error instanceof Error ? error.message : String(error)}`,
          );
        }
      }
    }
  }

  return {
    schema_version: RUNNER_SCHEMA_VERSION,
    benchmark_release: releaseMetadata.benchmarkRelease,
    dataset_release: releaseMetadata.datasetRelease,
    language: 'js',
    generated_at: new Date().toISOString(),
    filters: {
      variant: options.variant,
      size: options.size,
      shape: options.shape,
        case: options.case,
        warmups: options.warmups,
        iterations: options.iterations,
      },
    environment: environmentInfo(),
    dataset_count: datasets.length,
    discovered_datasets: datasets.map((dataset) => ({
      variant: dataset.variant,
      size: dataset.size,
      row_count: dataset.rowCount,
      meta_path: relativeToRoot(dataset.metaPath),
    })),
    results,
    issues: [...issueSet],
  };
}

function needsToonPackage(options, datasets) {
  if (options.variant && options.variant !== 'js-basic') {
    return false;
  }
  if (options.shape && options.shape !== 'structure') {
    return false;
  }
  if (options.case && !options.case.startsWith('toon_')) {
    return false;
  }
  return datasets.some((dataset) => dataset.variant === 'js-basic');
}

async function main() {
  const options = parseArgs(process.argv.slice(2));
  if (options.listCases) {
    process.stdout.write(`${JSON.stringify(listCaseEntries(options.variant, options.shape), null, 2)}\n`);
    return;
  }

  const payload = await runBenchmarks(options);
  process.stdout.write(`${JSON.stringify(payload, null, 2)}\n`);
}

await main();
