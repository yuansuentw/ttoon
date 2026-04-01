import { strict as assert } from 'node:assert';
import { describe, it } from 'node:test';
import * as api from '../index.js';
import { TranscodeError, tjsonToTtoon, ttoonToTjson } from '../index.js';

describe('direct transcode helpers', () => {
  it('tjsonToTtoon preserves object key insertion order', () => {
    const output = tjsonToTtoon('{"name": "Alice", "age": 30}');
    assert.equal(output, 'name: "Alice"\nage: 30\n');
  });

  it('ttoonToTjson preserves object key insertion order', () => {
    const output = ttoonToTjson('name: "Alice"\nage: 30\n');
    assert.equal(output, '{"name": "Alice", "age": 30}');
  });

  it('parse failure wraps as structured TranscodeError', () => {
    assert.throws(
      () => tjsonToTtoon('key: value'),
      (error: unknown) => {
        if (!(error instanceof TranscodeError)) {
          return false;
        }
        assert.equal(error.kind, 'transcode');
        assert.equal(error.operation, 'tjson_to_ttoon');
        assert.equal(error.phase, 'parse');
        assert.equal(error.sourceKind, 'parse');
        assert.equal(error.source.kind, 'parse');
        assert.match(error.message, /^tjson_to_ttoon: parse phase failed:/);
        return true;
      },
    );
  });

  it('root export no longer exposes low-level IR helpers', () => {
    assert.equal('parseTjsonStructure' in api, false);
    assert.equal('parseTtoon' in api, false);
    assert.equal('parseTtoonStructure' in api, false);
    assert.equal('serializeToTtoonStructure' in api, false);
    assert.equal('serializeToTtoonTabular' in api, false);
    assert.equal('irInt64' in api, false);
    assert.equal('isValidI64' in api, false);
  });
});
