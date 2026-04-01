# Datasets

> last update: 2026-04-01 12:10 +0800

- `source/authoritative/`: snapshots of source seed corpora
- `source/generated/`: intermediate output produced by deterministic generators
- `prepared/`: frozen fixtures consumed directly by benchmark runners

## Conventions

- Official dataset archives use the `.tar.zst` format.
- Runners read extracted directories under `prepared/<variant>/<size>/`.
- The `10k`, `100k`, and `1m` bundles are handled as external artifacts instead of long-lived Git-tracked files.
- Shell and Python benchmark entry points can auto-download missing archives from the configured `DATASET_BASE_URL`, verify SHA256 from the manifest, and unpack them into `prepared/<variant>/<size>/`.
- `object/source.json` uses the `column-oriented-nested-object-v1` source layout.
- Typed values in `extended/object/source.json` use the `typed-envelope-v1` transport shape.
- Run the validation command below to confirm that the frozen bundles reproduce the same content, metadata, and archive hashes.

## Build And Validation

```bash
uv run --project python python benchmarks/scripts/prepare_datasets.py
uv run --project python python benchmarks/scripts/prepare_datasets.py --validate-only --verify-reproducibility
uv run --project python python benchmarks/scripts/unpack_datasets.py
```
