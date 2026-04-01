import { strict as assert } from 'node:assert';
import { describe, it } from 'node:test';
import { parse } from '../index.js';
import { wasmParse } from '../wasm_adapter.js';

describe('ParseMode', () => {
  it('parse() defaults to compat in T-TOON path', () => {
    const result = parse('key: hello\n');
    assert.deepEqual(result, { key: 'hello' });
  });

  it('parse() defaults to compat fallback string for top-level bare token', () => {
    assert.equal(parse('@'), '@');
    assert.equal(parse('#'), '#');
    assert.equal(parse('$'), '$');
    assert.equal(parse('`'), '`');
  });

  it('parse() can be explicitly switched to strict', () => {
    assert.throws(
      () => parse('key: hello\n', { mode: 'strict' }),
      /unknown bare token/,
    );
  });

  it('wasmParse() defaults to compat, rejects bare string fallback in strict', () => {
    const compat = wasmParse('key: hello\n');
    assert.equal(compat.k, 'object');
    assert.equal(wasmParse('@').k, 'string');

    assert.throws(
      () => wasmParse('key: hello\n', 'strict'),
      /unknown bare token/,
    );
    assert.throws(
      () => wasmParse('@', 'strict'),
      /unknown bare token/,
    );
  });

  it('T-JSON path unaffected by mode', () => {
    assert.throws(
      () => parse('{"key": foo}', { mode: 'compat' }),
      /unknown keyword/,
    );
    assert.throws(
      () => parse('{"key": foo}', { mode: 'strict' }),
      /unknown keyword/,
    );
  });
});
