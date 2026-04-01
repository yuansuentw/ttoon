/**
 * Fixture-driven parse tests.
 * Shared test cases from tests/fixtures/parse_*.json
 *
 * JS parse() auto-detects tjson/ttoon.
 * Rust-specific parser-internal cases are skipped via "skip": ["js"].
 */
import { strict as assert } from 'node:assert';
import { describe, it } from 'node:test';
import { parse } from '../index.js';
import { wasmParse } from '../wasm_adapter.js';
import {
  loadFixture, shouldSkip, irFromFixture, assertIrEqual, nativeFromFixture, assertNativeEqual,
  type TypeTaggedValue,
} from './fixture_runner.js';

/**
 * Run parse tests for shared fixtures where JS parse() is expected
 * to align with fixture semantics.
 */
function runParseTests(fixtureName: string) {
  const fixture = loadFixture(fixtureName);

  describe(`fixture: ${fixtureName}`, () => {
    for (const test of fixture.tests) {
      if (shouldSkip(test, fixture)) continue;
      if (test.input == null) continue;

      it(`[${test.id}] ${test.description}`, () => {
        const parseOptions = test.mode ? { mode: test.mode } : undefined;
        if (test.expected && 'error' in test.expected) {
          assert.throws(() => parse(test.input!, parseOptions), `expected error for: ${test.input}`);
        } else if (test.expected) {
          const result = parse(test.input!, parseOptions);
          const expected = nativeFromFixture(test.expected as TypeTaggedValue);
          assertNativeEqual(result, expected, test.id);
        }
      });
    }
  });
}

// ─── Structure-format parse tests ───────────────────────────────────────────────

runParseTests('parse_scalars.json');
runParseTests('parse_integers.json');
runParseTests('parse_floats.json');
runParseTests('parse_strings.json');
runParseTests('parse_typed_cells.json');
runParseTests('parse_date_time.json');
runParseTests('parse_structures.json');

// ─── T-TOON Structure parse tests ──────────────────────────────────────────────

describe('fixture: parse_ttoon_structure.json', () => {
  const fixture = loadFixture('parse_ttoon_structure.json');

  for (const test of fixture.tests) {
    if (shouldSkip(test, fixture)) continue;

    it(`[${test.id}] ${test.description}`, () => {
      const mode = test.mode ?? 'compat';
      if (test.expected && 'error' in test.expected) {
        assert.throws(() => wasmParse(test.input!, mode));
      } else if (test.expected) {
        const result = wasmParse(test.input!, mode);
        const expected = irFromFixture(test.expected as TypeTaggedValue);
        assertIrEqual(result, expected, test.id);
      }
    });
  }
});

// ─── T-TOON Tabular parse tests (exact-count) ────────────────────────────────

function runTabularParseFixture(fixtureName: string) {
  const fixture = loadFixture(fixtureName);

  describe(`fixture: ${fixtureName}`, () => {
    for (const test of fixture.tests) {
      if (shouldSkip(test, fixture)) continue;

      it(`[${test.id}] ${test.description}`, () => {
        const mode = test.mode ?? 'compat';

        if (test.expected && 'error' in test.expected) {
          assert.throws(() => wasmParse(test.input!, mode));
        } else if (test.expected) {
          const result = wasmParse(test.input!, mode);
          const expected = irFromFixture(test.expected as TypeTaggedValue);
          assertIrEqual(result, expected, test.id);
        }
      });
    }
  });
}

runTabularParseFixture('parse_ttoon_tabular_exact.json');
runTabularParseFixture('parse_ttoon_tabular_streaming.json');

// ─── Error/Lex tests ────────────────────────────────────────────────────────────

describe('fixture: error_lex.json', () => {
  const fixture = loadFixture('error_lex.json');

  for (const test of fixture.tests) {
    if (shouldSkip(test, fixture)) continue;

    it(`[${test.id}] ${test.description}`, () => {
      const parseOptions = test.mode ? { mode: test.mode } : undefined;
      assert.throws(() => parse(test.input!, parseOptions), `expected lex error for: ${JSON.stringify(test.input)}`);
    });
  }
});

// ─── Validation error tests ─────────────────────────────────────────────────────

describe('fixture: validation_errors.json', () => {
  const fixture = loadFixture('validation_errors.json');

  for (const test of fixture.tests) {
    if (shouldSkip(test, fixture)) continue;

    it(`[${test.id}] ${test.description}`, () => {
      const parseOptions = test.mode ? { mode: test.mode } : undefined;
      assert.throws(() => parse(test.input!, parseOptions));
    });
  }
});

describe('fixture_runner native int conversion', () => {
  it('large int string converts to bigint without precision loss', () => {
    const result = nativeFromFixture({ type: 'int', value: '9223372036854775807' });
    assert.equal(result, 9223372036854775807n);
  });
});
