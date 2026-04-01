/**
 * Fixture-driven format detection tests.
 * Shared test cases from tests/fixtures/format_detect.json
 */
import { strict as assert } from 'node:assert';
import { describe, it } from 'node:test';
import { detectFormat, parse } from '../index.js';
import { loadFixture, shouldSkip } from './fixture_runner.js';

describe('fixture: format_detect', () => {
  const fixture = loadFixture('format_detect.json');

  for (const test of fixture.tests) {
    if (shouldSkip(test, fixture)) continue;

    it(`[${test.id}] ${test.description}`, () => {
      const result = detectFormat(test.input!);
      assert.equal(result, test.expected_format, `format mismatch for input: ${JSON.stringify(test.input)}`);

      // 若 fixture 標記 expected_parse_error，驗證解析確實失敗
      // （邊界 case：format_detect 正確路由，parser 拒絕不合法的語法）
      if (test.expected_parse_error) {
        assert.throws(
          () => parse(test.input!),
          (err: unknown) => {
            if (!(err instanceof Error)) return false;
            if (test.expected_parse_error_contains) {
              return err.message.includes(test.expected_parse_error_contains);
            }
            return true;
          },
          `[${test.id}] expected parse error but got success`,
        );
      }
    });
  }
});
