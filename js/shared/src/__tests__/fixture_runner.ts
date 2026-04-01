/**
 * Shared fixture runner for cross-language tests.
 * Loads JSON fixtures from tests/fixtures/ and provides helpers
 * for converting type-tagged values to IrNode / JS native values.
 */
import { readFileSync } from 'node:fs';
import { resolve, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';
import type { IrNode } from '../ir.js';
import {
  irNull, irBool, irInt, irInt64, irFloat, irDecimal,
  irStr, irDate, irTime, irDatetime, irUuid, irBinary, irList, irObject,
} from '../ir.js';

const __dirname = dirname(fileURLToPath(import.meta.url));
const FIXTURES_DIR = resolve(__dirname, '../../../../tests/fixtures');

export interface FixtureTest {
  [key: string]: unknown;
  id: string;
  description: string;
  mode?: 'compat' | 'strict';
  input?: string;
  input_variants?: string[];
  input_alt?: string;
  expected?: TypeTaggedValue | { error: string };
  expected_format?: string;
  expected_output?: string;
  expected_output_contains?: string[];
  expected_output_not_contains?: string[];
  expected_output_starts_with?: string;
  expected_num_rows?: number;
  expected_num_cols?: number;
  expected_schema?: Record<string, unknown>;
  expected_field_order?: string[];
  expected_rows?: TypeTaggedValue[];
  expected_row_count?: number;
  fields?: string[];
  rows?: Record<string, TypeTaggedValue>[];
  value?: TypeTaggedValue;
  format?: string;
  options?: Record<string, unknown>;
  expected_parse_error?: boolean;
  expected_parse_error_contains?: string;
  skip?: string[];
  parser?: string;
  roundtrip?: boolean;
  expect_equal?: boolean;
}

export interface Fixture {
  description: string;
  skip_default?: string[];
  tests: FixtureTest[];
}

export interface TypeTaggedValue {
  type: string;
  value?: unknown;
}

export function loadFixture(name: string): Fixture {
  const path = resolve(FIXTURES_DIR, name);
  const content = readFileSync(path, 'utf-8');
  return JSON.parse(content) as Fixture;
}

/** Should this test be skipped for JS? */
export function shouldSkip(test: FixtureTest, fixture?: Fixture): boolean {
  if (test.skip?.includes('js')) return true;
  if (fixture?.skip_default?.includes('js')) {
    // skip_default applies unless test explicitly overrides
    return true;
  }
  return false;
}

/** Convert type-tagged JSON value to IrNode. */
export function irFromFixture(obj: TypeTaggedValue): IrNode {
  switch (obj.type) {
    case 'null':
      return irNull();
    case 'bool':
      return irBool(obj.value as boolean);
    case 'int': {
      if (typeof obj.value === 'string') {
        const n = Number(obj.value);
        if (Number.isSafeInteger(n)) return irInt(n);
        return irInt64(BigInt(obj.value));
      }
      return irInt(obj.value as number);
    }
    case 'float': {
      if (typeof obj.value === 'string') {
        switch (obj.value) {
          case 'NaN': return irFloat(NaN);
          case '+Infinity': return irFloat(Infinity);
          case '-Infinity': return irFloat(-Infinity);
          case '-0.0': return irFloat(-0);
          default: throw new Error(`Unknown float special: ${obj.value}`);
        }
      }
      return irFloat(obj.value as number);
    }
    case 'decimal':
      return irDecimal(obj.value as string);
    case 'string':
      return irStr(obj.value as string);
    case 'date':
      return irDate(obj.value as string);
    case 'time':
      return irTime(obj.value as string);
    case 'datetime':
      return irDatetime(obj.value as string);
    case 'uuid':
      return irUuid(obj.value as string);
    case 'binary_hex': {
      const hex = obj.value as string;
      if (hex === '') return irBinary(new Uint8Array(0));
      const bytes = new Uint8Array(hex.length / 2);
      for (let i = 0; i < hex.length; i += 2) {
        bytes[i / 2] = parseInt(hex.substring(i, i + 2), 16);
      }
      return irBinary(bytes);
    }
    case 'binary_b64': {
      const b64 = obj.value as string;
      if (b64 === '') return irBinary(new Uint8Array(0));
      const binary = Buffer.from(b64, 'base64');
      return irBinary(new Uint8Array(binary));
    }
    case 'list': {
      const items = (obj.value as TypeTaggedValue[]).map(irFromFixture);
      return irList(items);
    }
    case 'object': {
      const entries = obj.value as Record<string, TypeTaggedValue>;
      const map = new Map<string, IrNode>();
      for (const [k, v] of Object.entries(entries)) {
        map.set(k, irFromFixture(v));
      }
      return irObject(map);
    }
    default:
      throw new Error(`Unknown type: ${obj.type}`);
  }
}

/** Convert type-tagged JSON value to JS native value. */
export function nativeFromFixture(obj: TypeTaggedValue | { error: string }): unknown {
  if ('error' in obj) {
    throw new Error(`Cannot convert error to native: ${obj.error}`);
  }
  const tv = obj as TypeTaggedValue;
  switch (tv.type) {
    case 'null': return null;
    case 'bool': return tv.value as boolean;
    case 'int': {
      if (typeof tv.value === 'string') return BigInt(tv.value);
      return tv.value as number;
    }
    case 'float': {
      if (typeof tv.value === 'string') {
        switch (tv.value) {
          case 'NaN': return NaN;
          case '+Infinity': return Infinity;
          case '-Infinity': return -Infinity;
          case '-0.0': return -0;
          default: throw new Error(`Unknown float: ${tv.value}`);
        }
      }
      return tv.value as number;
    }
    case 'decimal': {
      // JS default decimal mode is 'string' — strip 'm' suffix
      const s = tv.value as string;
      return s.endsWith('m') ? s.slice(0, -1) : s;
    }
    case 'string': return tv.value as string;
    case 'date': return tv.value as string;
    case 'time': return tv.value as string;
    case 'datetime': return tv.value as string;
    case 'uuid': return tv.value as string;
    case 'binary_hex': {
      const hex = tv.value as string;
      if (hex === '') return new Uint8Array(0);
      const bytes = new Uint8Array(hex.length / 2);
      for (let i = 0; i < hex.length; i += 2) {
        bytes[i / 2] = parseInt(hex.substring(i, i + 2), 16);
      }
      return bytes;
    }
    case 'binary_b64': {
      const b64 = tv.value as string;
      if (b64 === '') return new Uint8Array(0);
      return new Uint8Array(Buffer.from(b64, 'base64'));
    }
    case 'list':
      return (tv.value as TypeTaggedValue[]).map(nativeFromFixture);
    case 'object': {
      const entries = tv.value as Record<string, TypeTaggedValue>;
      const result: Record<string, unknown> = {};
      for (const [k, v] of Object.entries(entries)) {
        result[k] = nativeFromFixture(v);
      }
      return result;
    }
    default:
      throw new Error(`Unknown type: ${tv.type}`);
  }
}

/**
 * Deep-compare two JS native values, handling NaN, -0, and Uint8Array.
 */
export function assertNativeEqual(actual: unknown, expected: unknown, context: string): void {
  if (typeof expected === 'number') {
    if (Number.isNaN(expected)) {
      if (!(typeof actual === 'number' && Number.isNaN(actual))) {
        throw new Error(`${context}: expected NaN, got ${String(actual)}`);
      }
      return;
    }
    if (Object.is(expected, -0)) {
      if (!(typeof actual === 'number' && Object.is(actual, -0))) {
        throw new Error(`${context}: expected -0, got ${String(actual)}`);
      }
      return;
    }
  }

  if (expected instanceof Uint8Array) {
    if (!(actual instanceof Uint8Array)) {
      throw new Error(`${context}: expected Uint8Array, got ${typeof actual}`);
    }
    if (actual.length !== expected.length) {
      throw new Error(`${context}: binary length ${actual.length} !== ${expected.length}`);
    }
    for (let i = 0; i < expected.length; i++) {
      if (actual[i] !== expected[i]) {
        throw new Error(`${context}: binary[${i}] ${actual[i]} !== ${expected[i]}`);
      }
    }
    return;
  }

  if (Array.isArray(expected)) {
    if (!Array.isArray(actual)) {
      throw new Error(`${context}: expected array, got ${typeof actual}`);
    }
    if (actual.length !== expected.length) {
      throw new Error(`${context}: list length ${actual.length} !== ${expected.length}`);
    }
    for (let i = 0; i < expected.length; i++) {
      assertNativeEqual(actual[i], expected[i], `${context}[${i}]`);
    }
    return;
  }

  if (expected !== null && typeof expected === 'object') {
    if (
      actual === null ||
      typeof actual !== 'object' ||
      Array.isArray(actual) ||
      actual instanceof Uint8Array
    ) {
      throw new Error(`${context}: expected object, got ${typeof actual}`);
    }
    const expectedObj = expected as Record<string, unknown>;
    const actualObj = actual as Record<string, unknown>;
    const expectedKeys = Object.keys(expectedObj);
    const actualKeys = Object.keys(actualObj);
    if (actualKeys.length !== expectedKeys.length) {
      throw new Error(`${context}: object key count ${actualKeys.length} !== ${expectedKeys.length}`);
    }
    for (let i = 0; i < expectedKeys.length; i++) {
      if (actualKeys[i] !== expectedKeys[i]) {
        throw new Error(`${context}: object key order mismatch: ${actualKeys.join(',')} !== ${expectedKeys.join(',')}`);
      }
    }
    for (const key of expectedKeys) {
      assertNativeEqual(actualObj[key], expectedObj[key], `${context}.${key}`);
    }
    return;
  }

  if (!Object.is(actual, expected)) {
    throw new Error(`${context}: ${JSON.stringify(actual)} !== ${JSON.stringify(expected)}`);
  }
}

/** Deep-compare two IrNode values, handling NaN and Uint8Array. */
export function assertIrEqual(actual: IrNode, expected: IrNode, context: string): void {
  if (actual.k !== expected.k) {
    throw new Error(`${context}: kind mismatch: ${actual.k} !== ${expected.k}`);
  }

  switch (expected.k) {
    case 'null':
      break;
    case 'float': {
      const av = (actual as { k: 'float'; v: number }).v;
      const ev = expected.v;
      if (Number.isNaN(ev)) {
        if (!Number.isNaN(av)) {
          throw new Error(`${context}: expected NaN, got ${av}`);
        }
      } else if (Object.is(ev, -0)) {
        if (!Object.is(av, -0)) {
          throw new Error(`${context}: expected -0, got ${av}`);
        }
      } else if (av !== ev) {
        throw new Error(`${context}: ${av} !== ${ev}`);
      }
      break;
    }
    case 'binary': {
      const av = (actual as { k: 'binary'; v: Uint8Array }).v;
      const ev = expected.v;
      if (av.length !== ev.length) {
        throw new Error(`${context}: binary length ${av.length} !== ${ev.length}`);
      }
      for (let i = 0; i < av.length; i++) {
        if (av[i] !== ev[i]) {
          throw new Error(`${context}: binary[${i}] ${av[i]} !== ${ev[i]}`);
        }
      }
      break;
    }
    case 'list': {
      const al = (actual as { k: 'list'; v: IrNode[] }).v;
      const el = expected.v;
      if (al.length !== el.length) {
        throw new Error(`${context}: list length ${al.length} !== ${el.length}`);
      }
      for (let i = 0; i < al.length; i++) {
        assertIrEqual(al[i]!, el[i]!, `${context}[${i}]`);
      }
      break;
    }
    case 'object': {
      const am = (actual as { k: 'object'; v: Map<string, IrNode> }).v;
      const em = expected.v;
      if (am.size !== em.size) {
        throw new Error(`${context}: object size ${am.size} !== ${em.size}`);
      }
      const actualKeys = Array.from(am.keys());
      const expectedKeys = Array.from(em.keys());
      for (let i = 0; i < expectedKeys.length; i++) {
        if (actualKeys[i] !== expectedKeys[i]) {
          throw new Error(`${context}: object key order mismatch: ${actualKeys.join(',')} !== ${expectedKeys.join(',')}`);
        }
      }
      for (const [k, ev] of em) {
        const av = am.get(k);
        if (!av) {
          throw new Error(`${context}: missing key '${k}'`);
        }
        assertIrEqual(av, ev, `${context}.${k}`);
      }
      break;
    }
    case 'int': {
      const an = actual as { k: 'int'; v: number; v64?: bigint };
      const en = expected as { k: 'int'; v: number; v64?: bigint };
      // v64 比較
      if (en.v64 !== undefined) {
        if (an.v64 !== en.v64) {
          throw new Error(`${context}: v64 ${an.v64} !== ${en.v64}`);
        }
        // v 為 NaN sentinel — 兩邊都應為 NaN
        if (!Number.isNaN(an.v)) {
          throw new Error(`${context}: expected v=NaN for overflow int, got ${an.v}`);
        }
      } else {
        if (an.v64 !== undefined) {
          throw new Error(`${context}: unexpected v64=${an.v64}`);
        }
        if (an.v !== en.v) {
          throw new Error(`${context}: ${an.v} !== ${en.v}`);
        }
      }
      break;
    }
    default: {
      const av = (actual as { v?: unknown }).v;
      const ev = (expected as { v?: unknown }).v;
      if (av !== ev) {
        throw new Error(`${context}: ${JSON.stringify(av)} !== ${JSON.stringify(ev)}`);
      }
    }
  }
}
