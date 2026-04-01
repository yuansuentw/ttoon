#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"

cd "$ROOT_DIR"

expected_version="${1:-}"

python_version="$(awk -F'"' '/^version = "/ {print $2; exit}' python/pyproject.toml)"
python_requires="$(awk -F'"' '/^requires-python = "/ {print $2; exit}' python/pyproject.toml)"
rust_core_version="$(awk -F'"' '/^version = "/ {print $2; exit}' rust/crates/core/Cargo.toml)"
js_shared_version="$(node -p "require('./js/shared/package.json').version")"
js_node_version="$(node -p "require('./js/node/package.json').version")"
js_web_version="$(node -p "require('./js/web/package.json').version")"

versions=(
  "$python_version"
  "$rust_core_version"
  "$js_shared_version"
  "$js_node_version"
  "$js_web_version"
)

reference_version="${versions[0]}"
for version in "${versions[@]}"; do
  if [[ "$version" != "$reference_version" ]]; then
    echo "version mismatch detected: ${versions[*]}" >&2
    exit 1
  fi
done

if [[ -n "$expected_version" && "$reference_version" != "$expected_version" ]]; then
  echo "tag version '$expected_version' does not match package version '$reference_version'" >&2
  exit 1
fi

if [[ "$python_requires" != ">=3.11" ]]; then
  echo "python requires-python must be >=3.11 for abi3-py311 wheels" >&2
  exit 1
fi

if ! grep -q 'abi3-py311' rust/crates/py-bridge/Cargo.toml; then
  echo "ttoon-py-bridge must keep abi3-py311 enabled" >&2
  exit 1
fi

if ! grep -q '^publish = false$' rust/crates/py-bridge/Cargo.toml; then
  echo "rust/crates/py-bridge/Cargo.toml must set publish = false" >&2
  exit 1
fi

echo "release versions verified: $reference_version"
