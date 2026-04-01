# TTOON Test Fixtures

Cross-language test data for Rust, JavaScript, and Python implementations.

## Schema

Each fixture file is a JSON file with this structure:

```json
{
  "description": "Category description",
  "tests": [
    {
      "id": "unique_test_id",
      "description": "What this test verifies",
      "mode": "compat",
      "input": "the TTOON text to parse",
      "expected": { "type": "int", "value": 123 }
    }
  ]
}
```

### Type-Tagged Values

JSON cannot express TTOON's rich type system, so we use `{"type": "<kind>", "value": <val>}`:

| Type | Example |
|------|---------|
| `null` | `{"type": "null"}` |
| `bool` | `{"type": "bool", "value": true}` |
| `int` | `{"type": "int", "value": 123}` |
| `float` | `{"type": "float", "value": 3.14}` |
| `string` | `{"type": "string", "value": "hello"}` |
| `date` | `{"type": "date", "value": "2024-01-01"}` |
| `time` | `{"type": "time", "value": "14:30:00"}` |
| `datetime` | `{"type": "datetime", "value": "2024-01-01T00:00:00Z"}` |
| `decimal` | `{"type": "decimal", "value": "123.45m"}` |
| `uuid` | `{"type": "uuid", "value": "426ac144-..."}` |
| `binary_hex` | `{"type": "binary_hex", "value": "0A1B2C"}` |
| `binary_b64` | `{"type": "binary_b64", "value": "YWJj"}` |
| `list` | `{"type": "list", "value": [...]}` |
| `object` | `{"type": "object", "value": {"key": {...}}}` |

### Special Values

- Large integers (> 2^53): use string `{"type": "int", "value": "9223372036854775807"}`
- Special floats: `{"type": "float", "value": "NaN"}`, `"+Infinity"`, `"-Infinity"`, `"-0.0"`
- Errors: `{"error": "parse_error"}`, `{"error": "lex_error"}`, `{"error": "arrow_error"}`

### Test Variants

**Parse tests** (`parse_*.json`):
```json
{ "id": "...", "mode": "strict", "input": "[123]", "expected": {"type": "list", "value": [{"type": "int", "value": 123}]} }
```

The `mode` field only affects the T-TOON and `typed_unit` parsing paths. Use `"compat"` or `"strict"`. If omitted, each language uses its public API default.

**Format detection** (`format_detect.json`):
```json
{ "id": "...", "input": "[1, 2]", "expected_format": "tjson" }
```

**Roundtrip tests** (`roundtrip.json`):
```json
{ "id": "...", "value": {"type": "int", "value": 123}, "format": "tjson" }
```

**Serialize options** (`serialize_options.json`):
```json
{ "id": "...", "value": {...}, "options": {"binary_format": "b64"}, "expected_output": "[b64(YQ==)]" }
```

**Tabular serialize fixtures**:
- `serialize_ttoon_tabular_exact.json`: batch or known-count T-TOON tabular serialization using `value`
- `serialize_ttoon_tabular_streaming.json`: streaming T-TOON tabular serialization using `fields` and `rows`

**Validation tests** (`validation_errors.json`):
```json
{ "id": "...", "input": "[-0m]", "expected": {"error": "parse_error"} }
```

**Arrow batch tests** (`read_arrow.json`):
```json
{
  "id": "...",
  "input": "[{\"a\": 1}, {\"b\": 2}]",
  "expected_num_rows": 2,
  "expected_num_cols": 2,
  "expected_field_order": ["a", "b"],
  "expected_schema": {
    "a": { "type": "int64" },
    "b": { "type": "int64" }
  },
  "expected_rows": [
    { "type": "object", "value": { "a": {"type": "int", "value": 1}, "b": {"type": "null"} } },
    { "type": "object", "value": { "a": {"type": "null"}, "b": {"type": "int", "value": 2} } }
  ]
}
```

`read_arrow.json` contract:
- T-JSON list of objects allows sparse rows; missing keys are treated as null.
- T-TOON tabular follows explicit header order and width.
- Row values must remain 2D tabular scalars; nested object/list values are rejected.

### Skipping Languages

Use `"skip": ["js"]` to skip a test for a specific language when that language has a known, justified limitation, such as JavaScript `Number` range limits. Do not use `skip` as a generic "not yet implemented" marker.

### Format Names

Fixture files use these format names (v003):

| Fixture | Rust | JS | Python |
|---------|------|-----|--------|
| `"tjson"` | `Format::Tjson` | `"tjson"` | `"tjson"` |
| `"ttoon"` | `Format::Ttoon` | `"ttoon"` | `"ttoon"` |
| `"typed_unit"` | `Format::TypedUnit` | `"typed_unit"` | `"typed_unit"` |

## Adding New Test Cases

1. Add the test case to the appropriate fixture file
2. Run all three language test suites to verify:
   ```bash
   cd rust && cargo test
   cd js/shared && npm test
   cd python && uv run python -m pytest
   ```
3. All languages will automatically pick up the new case

## Fixture Files

| File | Description |
|------|-------------|
| `format_detect.json` | Format detection (input -> format name) |
| `parse_scalars.json` | Null, bool, keyword parsing |
| `parse_integers.json` | Integer parsing and edge cases |
| `parse_floats.json` | Float parsing, special values, scientific notation |
| `parse_strings.json` | String parsing, escapes, Unicode |
| `parse_typed_cells.json` | UUID, hex, base64, decimal typed cells |
| `parse_date_time.json` | Date, time, datetime parsing |
| `parse_structures.json` | Lists, objects, and nesting through the public parse API, including empty-input and whitespace-only object behavior |
| `parse_ttoon_structure.json` | T-TOON key:value structure format |
| `parse_ttoon_tabular_exact.json` | T-TOON tabular exact-count `[N]` parse contract |
| `parse_ttoon_tabular_streaming.json` | T-TOON tabular streaming `[*]` parse contract |
| `serialize_options.json` | Serializer options (binary format, indent size, delimiter) |
| `serialize_ttoon_tabular_exact.json` | T-TOON tabular exact-count `[N]` serialize contract |
| `serialize_ttoon_tabular_streaming.json` | T-TOON tabular streaming `[*]` serialize contract |
| `roundtrip.json` | Serialize -> parse roundtrip verification |
| `validation_errors.json` | Validation error cases |
| `error_lex.json` | Lexer error cases |
