/**
 * Arrow 整合測試
 * 執行：node --import tsx --test src/__tests__/arrow_convert.test.ts
 */
import { strict as assert } from 'node:assert';
import { describe, it } from 'node:test';
import * as arrow from 'apache-arrow';
import type { DataType } from 'apache-arrow';
import { isArrowTable, parse, readArrow, stringifyArrow, stringifyArrowTjson } from '../index.js';
import {
  assertNativeEqual,
  loadFixture,
  nativeFromFixture,
  shouldSkip,
  type TypeTaggedValue,
} from './fixture_runner.js';

interface SchemaSpecObject {
  type: string;
  nullable?: boolean;
  precision?: number;
  scale?: number;
  unit?: string;
  timezone?: string;
}

type SchemaSpec = string | SchemaSpecObject;

const LEGACY_SCHEMA_TYPE_MAP: Record<string, SchemaSpecObject> = {
  int: { type: 'int64' },
  int_nullable: { type: 'int64', nullable: true },
  float: { type: 'float64' },
  float_nullable: { type: 'float64', nullable: true },
  string: { type: 'utf8' },
  string_nullable: { type: 'utf8', nullable: true },
  bool: { type: 'bool' },
  bool_nullable: { type: 'bool', nullable: true },
};

function normalizeSchemaSpec(spec: SchemaSpec): SchemaSpecObject {
  return typeof spec === 'string' ? (LEGACY_SCHEMA_TYPE_MAP[spec] ?? { type: spec }) : spec;
}

function assertFieldSchema(field: { name: string; type: DataType; nullable: boolean; metadata?: Map<string, string> }, spec: SchemaSpec) {
  const normalized = normalizeSchemaSpec(spec);

  switch (normalized.type) {
    case 'null':
      assert.equal(field.type.toString(), 'Null');
      break;
    case 'bool':
      assert.equal(field.type.toString(), 'Bool');
      break;
    case 'int64':
      assert.equal(field.type.toString(), 'Int64');
      break;
    case 'float64':
      assert.equal(field.type.toString(), 'Float64');
      break;
    case 'utf8':
      assert.equal(field.type.toString(), 'Utf8');
      break;
    case 'binary':
      assert.equal(field.type.toString(), 'Binary');
      break;
    case 'uuid':
      assert.equal(field.type.toString(), 'FixedSizeBinary[16]');
      assert.equal((field.type as { byteWidth?: number }).byteWidth, 16);
      assert.equal(field.metadata?.get('ARROW:extension:name'), 'arrow.uuid');
      break;
    case 'date32':
      assert.equal(field.type.toString(), 'Date32<DAY>');
      break;
    case 'time64':
      assert.equal(field.type.toString(), 'Time64<MICROSECOND>');
      assert.equal((field.type as { unit?: number }).unit, 2);
      break;
    case 'timestamp': {
      const expected = normalized.timezone
        ? 'Timestamp<MICROSECOND, UTC>'
        : 'Timestamp<MICROSECOND>';
      assert.equal(field.type.toString(), expected);
      assert.equal((field.type as { unit?: number }).unit, 2);
      assert.equal((field.type as { timezone?: string | null }).timezone ?? undefined, normalized.timezone);
      break;
    }
    case 'decimal128':
    case 'decimal256': {
      const expectedScale = normalized.scale ?? 0;
      const expectedPrecision = normalized.precision ?? 0;
      const sign = expectedScale > 0 ? '+' : '';
      assert.equal(field.type.toString(), `Decimal[${expectedPrecision}e${sign}${expectedScale}]`);
      assert.equal((field.type as { bitWidth?: number }).bitWidth, normalized.type === 'decimal128' ? 128 : 256);
      assert.equal((field.type as { precision?: number }).precision, expectedPrecision);
      assert.equal((field.type as { scale?: number }).scale, expectedScale);
      break;
    }
    default:
      throw new Error(`unknown schema type: ${normalized.type}`);
  }

  if (normalized.nullable !== undefined) {
    assert.equal(field.nullable, normalized.nullable, `${field.name} nullable`);
  }
}

function normalizeArrowScalar(value: unknown): unknown {
  if (typeof value === 'bigint') {
    if (value <= BigInt(Number.MAX_SAFE_INTEGER) && value >= BigInt(Number.MIN_SAFE_INTEGER)) {
      return Number(value);
    }
    return value;
  }
  return value;
}

function tableRowsToNative(table: arrow.Table): Array<Record<string, unknown>> {
  const fieldNames = table.schema.fields.map(field => field.name);
  const columns = fieldNames.map(fieldName => table.getChild(fieldName)!);

  return Array.from({ length: table.numRows }, (_, rowIdx) => {
    const row: Record<string, unknown> = {};
    for (let colIdx = 0; colIdx < fieldNames.length; colIdx++) {
      row[fieldNames[colIdx]!] = normalizeArrowScalar(columns[colIdx]!.get(rowIdx));
    }
    return row;
  });
}

describe('Arrow public API roundtrip', () => {
  it('T-TOON Tabular → Arrow Table → T-TOON Tabular', async () => {
    const input = `[3]{name,age,active}:\nAlice, 30, true\nBob, 25, false\nCarol, 28, true\n`;

    const table = await readArrow(input);
    assert.equal(table.numRows, 3);

    const output = await stringifyArrow(table);
    const reparsed = await readArrow(output);
    assert.equal(reparsed.numRows, 3);
    assert.deepEqual(reparsed.schema.fields.map(f => f.name), ['name', 'age', 'active']);
  });
});

describe('isArrowTable', () => {
  it('Arrow Table returns true', () => {
    const table = new arrow.Table({
      x: arrow.vectorFromArray([1n], new arrow.Int64()),
    });
    assert.equal(isArrowTable(table), true);
  });

  it('non-Table objects return false', () => {
    assert.equal(isArrowTable({}), false);
    assert.equal(isArrowTable(null), false);
    assert.equal(isArrowTable(42), false);
    assert.equal(isArrowTable([1, 2, 3]), false);
  });
});

describe('readArrow / stringifyArrow', () => {
  it('readArrow preserves schema field order in T-JSON path', async () => {
    const input = '[{"name": "Alice", "age": 30, "active": true}]';
    const table = await readArrow(input);
    assert.deepEqual(table.schema.fields.map(f => f.name), ['name', 'age', 'active']);
  });

  it('readArrow returns Arrow Table', async () => {
    const input = `[2]{name,score}:\nAlice, 95\nBob, 87\n`;
    const table = await readArrow(input);
    assert.equal(table.numRows, 2);

    const nameCol = table.getChild('name')!;
    assert.equal(nameCol.get(0), 'Alice');
    assert.equal(nameCol.get(1), 'Bob');

    const scoreCol = table.getChild('score')!;
    assert.equal(scoreCol.get(0), 95n);
    assert.equal(scoreCol.get(1), 87n);
  });

  it('stringifyArrow outputs T-TOON Tabular', async () => {
    const input = `[2]{name,score}:\nAlice, 95\nBob, 87\n`;
    const table = await readArrow(input);
    const output = await stringifyArrow(table);

    assert.ok(output.includes('{'));
    assert.ok(output.includes('name'));
    assert.ok(output.includes('score'));
    assert.ok(output.includes('Alice'));
    assert.ok(output.includes('Bob'));
  });

  it('stringifyArrowTjson outputs T-JSON list-of-objects', async () => {
    const input = `[2]{name,score,active}:\nAlice, 95, true\nBob, 87, false\n`;
    const table = await readArrow(input);
    const output = await stringifyArrowTjson(table);

    assert.ok(output.startsWith('['));
    assert.ok(output.includes('"name"'));
    assert.ok(output.includes('"score"'));
    assert.ok(output.includes('"active"'));

    const reparsed = parse<Array<Record<string, unknown>>>(output);
    assert.deepEqual(reparsed, [
      { name: 'Alice', score: 95, active: true },
      { name: 'Bob', score: 87, active: false },
    ]);
  });

  it('stringifyArrow preserves Int64 precision beyond safe integer', async () => {
    const table = new arrow.Table({
      id: arrow.vectorFromArray([9007199254740993n], new arrow.Int64()),
    });

    const output = await stringifyArrow(table);
    assert.equal(output, `[1]{id}:\n9007199254740993\n`);
  });

  it('stringifyArrowTjson preserves Int64 precision beyond safe integer', async () => {
    const table = new arrow.Table({
      id: arrow.vectorFromArray([9007199254740993n], new arrow.Int64()),
    });

    const output = await stringifyArrowTjson(table);
    assert.equal(output, `[{"id": 9007199254740993}]`);
  });

  it('stringifyArrow preserves schema header for 0-row Arrow table', async () => {
    const table = new arrow.Table({
      a: arrow.vectorFromArray([], new arrow.Int64()),
      b: arrow.vectorFromArray([], new arrow.Utf8()),
    });

    const output = await stringifyArrow(table);
    assert.equal(output, `[0]{a,b}:\n`);
  });

  it('stringifyArrow retains all data for multi-batch Table', async () => {
    const table1 = new arrow.Table({
      name: arrow.vectorFromArray(['Alice'], new arrow.Utf8()),
      age: arrow.vectorFromArray([1n], new arrow.Int64()),
    });
    const table2 = new arrow.Table({
      name: arrow.vectorFromArray(['Bob'], new arrow.Utf8()),
      age: arrow.vectorFromArray([2n], new arrow.Int64()),
    });
    const multiBatch = table1.concat(table2);
    // 確認是真正的 multi-batch（2 batches, 各 1 row）
    assert.equal(multiBatch.batches.length, 2);
    assert.equal(multiBatch.numRows, 2);

    const output = await stringifyArrow(multiBatch);
    const reparsed = await readArrow(output);
    assert.equal(reparsed.numRows, 2, 'multi-batch Table should retain all rows after stringifyArrow roundtrip');
  });

  it('stringifyArrowTjson retains all data for multi-batch Table', async () => {
    const table1 = new arrow.Table({
      name: arrow.vectorFromArray(['Alice'], new arrow.Utf8()),
      age: arrow.vectorFromArray([1n], new arrow.Int64()),
    });
    const table2 = new arrow.Table({
      name: arrow.vectorFromArray(['Bob'], new arrow.Utf8()),
      age: arrow.vectorFromArray([2n], new arrow.Int64()),
    });
    const multiBatch = table1.concat(table2);
    assert.equal(multiBatch.batches.length, 2);

    const output = await stringifyArrowTjson(multiBatch);
    const reparsed = parse<Array<Record<string, unknown>>>(output);
    assert.equal(reparsed.length, 2, 'multi-batch Table should retain all rows after stringifyArrowTjson roundtrip');
    assert.deepEqual(reparsed, [
      { name: 'Alice', age: 1 },
      { name: 'Bob', age: 2 },
    ]);
  });
});

describe('fixture: read_arrow.json', () => {
  const fixture = loadFixture('read_arrow.json');

  for (const test of fixture.tests) {
    if (shouldSkip(test, fixture)) continue;

    it(`[${test.id}] ${test.description}`, async () => {
      const expected = test.expected as { error?: string } | undefined;

      if (expected?.error) {
        await assert.rejects(
          () => readArrow(test.input!),
          `expected ${expected.error} for: ${test.input}`,
        );
        return;
      }

      const table = await readArrow(test.input!);

      if (test.expected_num_rows !== undefined) {
        assert.equal(table.numRows, test.expected_num_rows, 'numRows');
      }
      if (test.expected_num_cols !== undefined) {
        assert.equal(table.schema.fields.length, test.expected_num_cols, 'numCols');
      }
      if (test.expected_field_order) {
        assert.deepEqual(table.schema.fields.map(field => field.name), test.expected_field_order, 'fieldOrder');
      }
      if (test.expected_schema) {
        const schema = test.expected_schema as Record<string, SchemaSpec>;
        for (const [fieldName, spec] of Object.entries(schema)) {
          const field = table.schema.fields.find(f => f.name === fieldName);
          assert.ok(field, `field '${fieldName}' not found in schema`);
          assertFieldSchema(field, spec);
        }
      }
      if (test.expected_rows) {
        const expectedRows = test.expected_rows.map(row => nativeFromFixture(row as TypeTaggedValue));
        assertNativeEqual(tableRowsToNative(table), expectedRows, `${test.id}.rows`);
      }
    });
  }
});
