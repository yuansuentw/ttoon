/**
 * Fixture-driven serializer and roundtrip tests.
 * Shared test cases from tests/fixtures/serialize_options.json and roundtrip.json
 *
 * 序列化走 WASM bridge（wasmStringifyTtoon / wasmStringifyTjson），
 * 與 index.ts 公開 API 使用同一生產路徑。
 */
import { strict as assert } from 'node:assert';
import { describe, it } from 'node:test';
import type { SerializeOptions } from '../ttoon_serializer.js';
import {
  wasmParse,
  wasmStringifyTtoon,
  wasmStringifyTjson,
} from '../wasm_adapter.js';
import { StreamSchema, types, streamWriter } from '../index.js';
import {
  loadFixture, shouldSkip, irFromFixture, assertIrEqual,
  type TypeTaggedValue,
} from './fixture_runner.js';

/**
 * WASM 序列化器與舊 TS 序列化器的已知行為差異。
 * 這些 fixture 期待舊 TS 行為，WASM 路徑有不同語意：
 *  - serialize_binary_format_invalid: TS 在 JS 端驗證選項並拋錯；WASM 透傳到 Rust 端處理
 *  - serialize_tabular_pipe/tab_delimiter: TS 以 .sort() 排序欄位；WASM 保留 wire 順序
 */
const WASM_BEHAVIOR_DIFF = new Set([
  'serialize_binary_format_invalid',
]);

/** 從 fixture options 物件建構 SerializeOptions（忽略 format 與不支援的選項） */
function buildSerializeOptions(opts?: Record<string, unknown>): SerializeOptions {
  if (!opts) return {};
  const result: SerializeOptions = {};
  if (opts.delimiter !== undefined) result.delimiter = opts.delimiter as ',' | '\t' | '|';
  if (opts.indent_size !== undefined) result.indentSize = opts.indent_size as number;
  if (opts.binary_format !== undefined) result.binaryFormat = opts.binary_format as 'hex' | 'b64';
  return result;
}

// ─── Roundtrip tests ────────────────────────────────────────────────────────────

describe('fixture: roundtrip.json', () => {
  const fixture = loadFixture('roundtrip.json');

  for (const test of fixture.tests) {
    if (shouldSkip(test, fixture)) continue;

    it(`[${test.id}] ${test.description}`, () => {
      const irNode = irFromFixture(test.value as TypeTaggedValue);
      const format = test.format ?? 'tjson';
      const opts = buildSerializeOptions(test.options as Record<string, unknown> | undefined);

      if (format === 'ttoon') {
        const serialized = wasmStringifyTtoon(irNode, opts);
        const reparsed = wasmParse(serialized);
        assertIrEqual(reparsed, irNode, test.id);
        return;
      }

      // Default: tjson format — full serialize→parse roundtrip
      const serialized = wasmStringifyTjson(irNode, opts);
      const reparsed = wasmParse(serialized);
      assertIrEqual(reparsed, irNode, test.id);
    });
  }
});

// ─── Serialize options tests ────────────────────────────────────────────────────

describe('fixture: serialize_options.json', () => {
  const fixture = loadFixture('serialize_options.json');

  for (const test of fixture.tests) {
    if (shouldSkip(test, fixture)) continue;
    if (WASM_BEHAVIOR_DIFF.has(test.id)) continue;

    it(`[${test.id}] ${test.description}`, () => {
      const irNode = irFromFixture(test.value as TypeTaggedValue);
      const opts = buildSerializeOptions(test.options as Record<string, unknown> | undefined);
      const format = (test.options?.format as string) ?? 'tjson';

      // Handle error cases
      if (test.expected && 'error' in test.expected) {
        if (format === 'ttoon') {
          assert.throws(() => wasmStringifyTtoon(irNode, opts), `[${test.id}] expected serialize error`);
        } else {
          assert.throws(() => wasmStringifyTjson(irNode, opts), `[${test.id}] expected serialize error`);
        }
        return;
      }

      if (format === 'ttoon') {
        const output = wasmStringifyTtoon(irNode, opts);

        if (test.expected_output_starts_with) {
          assert.ok(output.startsWith(test.expected_output_starts_with),
            `[${test.id}] expected output to start with ${JSON.stringify(test.expected_output_starts_with)}, got: ${JSON.stringify(output)}`);
        }
        if (test.expected_output_contains) {
          for (const s of test.expected_output_contains) {
            assert.ok(output.includes(s), `[${test.id}] expected output to contain ${JSON.stringify(s)}, got: ${JSON.stringify(output)}`);
          }
        }
        if (test.expected_output_not_contains) {
          for (const s of test.expected_output_not_contains) {
            assert.ok(!output.includes(s), `[${test.id}] expected output NOT to contain ${JSON.stringify(s)}, got: ${JSON.stringify(output)}`);
          }
        }
        return;
      }

      // Default: T-JSON serializer
      if (test.expected_output) {
        const output = wasmStringifyTjson(irNode);
        assert.strictEqual(output, test.expected_output, `${test.id}: output mismatch`);
      }
    });
  }
});

// ─── Serialize tabular exact-count tests ───────────────────────────────────────

describe('fixture: serialize_ttoon_tabular_exact.json', () => {
  const fixture = loadFixture('serialize_ttoon_tabular_exact.json');

  for (const test of fixture.tests) {
    if (shouldSkip(test, fixture)) continue;

    it(`[${test.id}] ${test.description}`, () => {
      const irNode = irFromFixture(test.value as TypeTaggedValue);
      const opts = buildSerializeOptions(test.options as Record<string, unknown> | undefined);

      const output = wasmStringifyTtoon(irNode, opts);
      assertSerializeOutput(test, output);
    });
  }
});

// ─── Serialize tabular streaming tests ─────────────────────────────────────────

/** Map IR type tag → FieldTypeSpec for streaming schema */
function irTypeToFieldType(irType: string) {
  switch (irType) {
    case 'string': return types.string;
    case 'int': return types.int;
    case 'float': return types.float;
    case 'bool': return types.bool;
    default: return types.string;
  }
}

describe('fixture: serialize_ttoon_tabular_streaming.json', () => {
  const fixture = loadFixture('serialize_ttoon_tabular_streaming.json');

  for (const test of fixture.tests) {
    if (shouldSkip(test, fixture)) continue;

    it(`[${test.id}] ${test.description}`, async () => {
      if (!test.fields || !test.rows) {
        throw new Error(`[${test.id}] streaming fixture missing fields/rows`);
      }
      const { fields, rows } = test;

      // Infer schema from fields + all rows (scan all for nullability)
      const schemaInput: Record<string, import('../schema.js').FieldTypeSpec> = {};
      for (const f of fields) {
        let hasNull = false;
        let fieldType = types.string;
        let foundType = false;
        for (const row of rows) {
          const cell = row[f];
          if (!cell) continue;
          if (cell.type === 'null') { hasNull = true; continue; }
          if (!foundType) { fieldType = irTypeToFieldType(cell.type); foundType = true; }
        }
        schemaInput[f] = hasNull ? fieldType.nullable() : fieldType;
      }
      const schema = new StreamSchema(schemaInput);

      // Collect output
      let output = '';
      const writer = streamWriter({ write(chunk: string) { output += chunk; } }, { schema });

      // Write rows as native JS objects
      for (const row of rows) {
        const jsRow: Record<string, unknown> = {};
        for (const f of fields) {
          const cell = row[f];
          if (!cell || cell.type === 'null') { jsRow[f] = null; continue; }
          jsRow[f] = cell.value;
        }
        writer.write(jsRow);
      }
      await writer.close();

      assertSerializeOutput(test, output);
    });
  }
});

/** Shared output assertion for serialize fixtures */
function assertSerializeOutput(test: Record<string, unknown>, output: string) {
  const id = test.id as string;
  if (test.expected_output != null) {
    assert.strictEqual(output.trimEnd(), (test.expected_output as string).trimEnd(),
      `[${id}] output mismatch`);
  }
  if (test.expected_output_starts_with != null) {
    assert.ok(output.startsWith(test.expected_output_starts_with as string),
      `[${id}] expected to start with ${JSON.stringify(test.expected_output_starts_with)}, got: ${JSON.stringify(output)}`);
  }
  if (test.expected_output_contains != null) {
    for (const s of test.expected_output_contains as string[]) {
      assert.ok(output.includes(s),
        `[${id}] expected to contain ${JSON.stringify(s)}, got: ${JSON.stringify(output)}`);
    }
  }
  if (test.expected_output_not_contains != null) {
    for (const s of test.expected_output_not_contains as string[]) {
      assert.ok(!output.includes(s),
        `[${id}] expected NOT to contain ${JSON.stringify(s)}, got: ${JSON.stringify(output)}`);
    }
  }
}
