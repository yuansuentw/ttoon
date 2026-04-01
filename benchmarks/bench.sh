#!/usr/bin/env bash

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BENCHMARK_ROOT="${REPO_ROOT}/benchmarks"
BENCHMARK_RELEASE_MANIFEST_PATH="${BENCHMARK_ROOT}/manifests/benchmark_release.sh"
DATASET_MANIFEST_PATH="${BENCHMARK_ROOT}/manifests/datasets.sh"

# shellcheck source=/dev/null
source "${BENCHMARK_RELEASE_MANIFEST_PATH}"
# shellcheck source=/dev/null
source "${DATASET_MANIFEST_PATH}"

LANGUAGE=""
VARIANT=""
SIZE=""
SHAPE=""
CASE_NAME=""
API_MODE="all"
WARMUPS="2"
ITERATIONS="20"
PREPARED_ROOT="${BENCHMARK_ROOT}/datasets/prepared"
TRACE_MEMORY="0"
LIST_DATASETS="0"
LIST_CASES="0"
DRY_RUN="0"
YES="0"
FORCE_INTERACTIVE=""
NO_BOOTSTRAP="0"

LANGUAGES=("js" "python" "rust")
VARIANTS=("js-basic" "extended")
SIZES=("10k" "100k" "1m")
SHAPES=("structure" "tabular")
API_MODES=("all" "batch" "streaming")

declare -A CASE_MATRIX=(
  ["python|js-basic|structure"]="json_serialize json_deserialize tjson_serialize tjson_deserialize ttoon_serialize ttoon_deserialize"
  ["python|js-basic|tabular"]="arrow_tjson_serialize arrow_tjson_deserialize arrow_tjson_stream_serialize arrow_tjson_stream_deserialize arrow_ttoon_serialize arrow_ttoon_deserialize arrow_ttoon_stream_serialize arrow_ttoon_stream_deserialize"
  ["python|extended|structure"]="ttoon_serialize ttoon_deserialize"
  ["python|extended|tabular"]="arrow_ttoon_serialize arrow_ttoon_deserialize arrow_tjson_serialize arrow_tjson_deserialize arrow_tjson_stream_serialize arrow_tjson_stream_deserialize arrow_ttoon_stream_serialize arrow_ttoon_stream_deserialize"
  ["js|js-basic|structure"]="json_serialize json_deserialize tjson_serialize tjson_deserialize ttoon_serialize ttoon_deserialize toon_serialize toon_deserialize"
  ["js|js-basic|tabular"]="arrow_tjson_serialize arrow_tjson_deserialize arrow_ttoon_serialize arrow_ttoon_deserialize"
  ["js|extended|structure"]="ttoon_serialize ttoon_deserialize"
  ["js|extended|tabular"]="arrow_ttoon_serialize arrow_ttoon_deserialize arrow_tjson_serialize arrow_tjson_deserialize"
  ["rust|js-basic|structure"]="json_serialize json_deserialize tjson_serialize tjson_deserialize ttoon_serialize ttoon_deserialize"
  ["rust|js-basic|tabular"]="arrow_tjson_serialize arrow_tjson_deserialize arrow_tjson_stream_serialize arrow_tjson_stream_deserialize arrow_ttoon_serialize arrow_ttoon_deserialize arrow_ttoon_stream_serialize arrow_ttoon_stream_deserialize"
  ["rust|extended|structure"]="ttoon_serialize ttoon_deserialize"
  ["rust|extended|tabular"]="arrow_ttoon_serialize arrow_ttoon_deserialize arrow_tjson_serialize arrow_tjson_deserialize arrow_tjson_stream_serialize arrow_tjson_stream_deserialize arrow_ttoon_stream_serialize arrow_ttoon_stream_deserialize"
)

case_matches_api_mode() {
  local case_name="$1"
  case "${API_MODE}" in
    all) return 0 ;;
    batch) [[ "${case_name}" != *_stream_* ]] ;;
    streaming) [[ "${case_name}" == *_stream_* ]] ;;
    *) return 1 ;;
  esac
}

usage() {
  cat <<'EOF'
Usage:
  ./benchmarks/bench.sh [options]

Options:
  --language <js|python|rust>
  --variant <js-basic|extended>
  --size <10k|100k|1m>
  --shape <structure|tabular>
  --case <case-name>
  --api <all|batch|streaming>
  --warmups <n>
  --iterations <n>
  --dataset-root <path>
  --interactive
  --non-interactive
  --no-bootstrap
  --yes
  --trace-memory
  --dry-run
  --list-datasets
  --list-cases
  -h, --help

Examples:
  ./benchmarks/bench.sh --language js --variant js-basic --size 10k --shape structure
  ./benchmarks/bench.sh --language rust --variant extended --size 100k --case arrow_ttoon_deserialize
  ./benchmarks/bench.sh --language python --variant js-basic --size 100k --shape tabular --api streaming
EOF
}

log() {
  printf '[bench] %s\n' "$*" >&2
}

die() {
  printf '[bench] error: %s\n' "$*" >&2
  exit 1
}

validate_release_manifests() {
  [[ "${BENCHMARK_RELEASE}" =~ ^[0-9]+\.[0-9]+$ ]] || die "invalid BENCHMARK_RELEASE format: ${BENCHMARK_RELEASE}"
  [[ "${BENCHMARK_DATASET_RELEASE}" =~ ^[0-9]+$ ]] || die "invalid BENCHMARK_DATASET_RELEASE format: ${BENCHMARK_DATASET_RELEASE}"
  [[ "${DATASET_RELEASE}" =~ ^[0-9]+$ ]] || die "invalid DATASET_RELEASE format: ${DATASET_RELEASE}"

  local benchmark_release_major="${BENCHMARK_RELEASE%%.*}"
  if [[ "${benchmark_release_major}" != "${BENCHMARK_DATASET_RELEASE}" ]]; then
    die "BENCHMARK_RELEASE major does not match BENCHMARK_DATASET_RELEASE: ${BENCHMARK_RELEASE} vs ${BENCHMARK_DATASET_RELEASE}"
  fi
  if [[ "${BENCHMARK_DATASET_RELEASE}" != "${DATASET_RELEASE}" ]]; then
    die "BENCHMARK_DATASET_RELEASE does not match DATASET_RELEASE: ${BENCHMARK_DATASET_RELEASE} vs ${DATASET_RELEASE}"
  fi
}

require_command() {
  command -v "$1" >/dev/null 2>&1 || die "missing required command: $1"
}

has_command() {
  command -v "$1" >/dev/null 2>&1
}

contains_value() {
  local needle="$1"
  shift
  local item
  for item in "$@"; do
    if [[ "${item}" == "${needle}" ]]; then
      return 0
    fi
  done
  return 1
}

display_path() {
  local absolute_path="$1"
  if [[ "${absolute_path}" == "${REPO_ROOT}/"* ]]; then
    printf '%s' "${absolute_path#${REPO_ROOT}/}"
    return
  fi
  printf '%s' "${absolute_path}"
}

dataset_archive_path() {
  local variant="$1" size="$2"
  printf '%s/%s/%s.tar.zst' "${PREPARED_ROOT}" "${variant}" "${size}"
}

dataset_dir_path() {
  local variant="$1" size="$2"
  printf '%s/%s/%s' "${PREPARED_ROOT}" "${variant}" "${size}"
}

sha256_file() {
  local target="$1"
  if has_command sha256sum; then
    sha256sum "${target}" | awk '{print $1}'
    return
  fi
  if has_command shasum; then
    shasum -a 256 "${target}" | awk '{print $1}'
    return
  fi
  die "missing sha256 tool (sha256sum or shasum)"
}

print_case_matrix() {
  local language_filter="${1:-}"
  local variant_filter="${2:-}"
  local shape_filter="${3:-}"
  local language
  local variant
  local shape

  for language in "${LANGUAGES[@]}"; do
    if [[ -n "${language_filter}" && "${language_filter}" != "${language}" ]]; then
      continue
    fi
    for variant in "${VARIANTS[@]}"; do
      if [[ -n "${variant_filter}" && "${variant_filter}" != "${variant}" ]]; then
        continue
      fi
      for shape in "${SHAPES[@]}"; do
        if [[ -n "${shape_filter}" && "${shape_filter}" != "${shape}" ]]; then
          continue
        fi
        local entry="${CASE_MATRIX["${language}|${variant}|${shape}"]-}"
        if [[ -z "${entry}" ]]; then
          continue
        fi
        local case_name
        for case_name in ${entry}; do
          if ! case_matches_api_mode "${case_name}"; then
            continue
          fi
          printf '%s\t%s\t%s\t%s\n' "${language}" "${variant}" "${shape}" "${case_name}"
        done
      done
    done
  done
}

collect_cases() {
  local language="$1"
  local variant="$2"
  local shape_filter="${3:-}"
  local -A seen=()
  local shape
  for shape in "${SHAPES[@]}"; do
    if [[ -n "${shape_filter}" && "${shape_filter}" != "${shape}" ]]; then
      continue
    fi
    local entry="${CASE_MATRIX["${language}|${variant}|${shape}"]-}"
    local case_name
    for case_name in ${entry}; do
      if ! case_matches_api_mode "${case_name}"; then
        continue
      fi
      if [[ -z "${seen["${case_name}"]-}" ]]; then
        seen["${case_name}"]="1"
        printf '%s\n' "${case_name}"
      fi
    done
  done
}

list_datasets() {
  local variant size archive_path dir_path archive_state dir_state
  printf 'variant\tsize\tarchive_status\tdir_status\n'
  for variant in "${VARIANTS[@]}"; do
    for size in "${SIZES[@]}"; do
      archive_path="$(dataset_archive_path "${variant}" "${size}")"
      dir_path="$(dataset_dir_path "${variant}" "${size}")"
      archive_state="missing"
      dir_state="missing"
      if [[ -f "${archive_path}" ]]; then
        archive_state="present"
      fi
      if [[ -f "${dir_path}/meta.json" ]]; then
        dir_state="present"
      fi
      printf '%s\t%s\t%s\t%s\n' "${variant}" "${size}" "${archive_state}" "${dir_state}"
    done
  done
}

prompt_choice() {
  local prompt_text="$1"
  shift
  local options=("$@")
  local selected=""
  local index

  while [[ -z "${selected}" ]]; do
    printf '%s\n' "${prompt_text}" >&2
    for index in "${!options[@]}"; do
      printf '  %d) %s\n' "$((index + 1))" "${options[index]}" >&2
    done
    read -r -p "> " selected || exit 1
    if [[ "${selected}" =~ ^[0-9]+$ ]] && (( selected >= 1 && selected <= ${#options[@]} )); then
      printf '%s' "${options[selected-1]}"
      return
    fi
    for index in "${!options[@]}"; do
      if [[ "${selected}" == "${options[index]}" ]]; then
        printf '%s' "${selected}"
        return
      fi
    done
    printf 'Invalid option, please try again.\n' >&2
    selected=""
  done
}

confirm_continue() {
  if [[ "${YES}" == "1" ]]; then
    return 0
  fi
  if [[ ! -t 0 ]]; then
    return 0
  fi

  local answer=""
  read -r -p "Confirm execution? [y/N] " answer || exit 1
  [[ "${answer}" == "y" || "${answer}" == "Y" ]]
}

validate_args() {
  if [[ -n "${LANGUAGE}" ]] && ! contains_value "${LANGUAGE}" "${LANGUAGES[@]}"; then
    die "unsupported language: ${LANGUAGE}"
  fi
  if [[ -n "${VARIANT}" ]] && ! contains_value "${VARIANT}" "${VARIANTS[@]}"; then
    die "unsupported variant: ${VARIANT}"
  fi
  if [[ -n "${SIZE}" ]] && ! contains_value "${SIZE}" "${SIZES[@]}"; then
    die "unsupported size: ${SIZE}"
  fi
  if [[ -n "${SHAPE}" ]] && ! contains_value "${SHAPE}" "${SHAPES[@]}"; then
    die "unsupported shape: ${SHAPE}"
  fi
  if [[ -n "${API_MODE}" ]] && ! contains_value "${API_MODE}" "${API_MODES[@]}"; then
    die "unsupported api mode: ${API_MODE}"
  fi
  [[ "${WARMUPS}" =~ ^[0-9]+$ ]] || die "warmups must be a non-negative integer"
  [[ "${ITERATIONS}" =~ ^[0-9]+$ ]] || die "iterations must be a positive integer"
  if (( ITERATIONS <= 0 )); then
    die "iterations must be > 0"
  fi
}

resolve_prompted_args() {
  local should_prompt="0"
  if [[ "${FORCE_INTERACTIVE}" == "1" ]]; then
    should_prompt="1"
  elif [[ -t 0 && ( -z "${LANGUAGE}" || -z "${VARIANT}" || -z "${SIZE}" ) ]]; then
    should_prompt="1"
  fi

  if [[ "${should_prompt}" != "1" ]]; then
    return
  fi

  if [[ -z "${LANGUAGE}" ]]; then
    LANGUAGE="$(prompt_choice "Select benchmark language" "${LANGUAGES[@]}")"
  fi
  if [[ -z "${VARIANT}" ]]; then
    VARIANT="$(prompt_choice "Select dataset variant" "${VARIANTS[@]}")"
  fi
  if [[ -z "${SIZE}" ]]; then
    SIZE="$(prompt_choice "Select dataset size" "${SIZES[@]}")"
  fi
  if [[ -z "${SHAPE}" ]]; then
    local shape_choice
    shape_choice="$(prompt_choice "Select shape (all lets the runner run all shapes for the dataset)" "all" "${SHAPES[@]}")"
    if [[ "${shape_choice}" != "all" ]]; then
      SHAPE="${shape_choice}"
    fi
  fi
  if [[ "${API_MODE}" == "all" ]]; then
    API_MODE="$(prompt_choice "Select API type" "${API_MODES[@]}")"
  fi
  if [[ -z "${CASE_NAME}" ]]; then
    local case_options=("all")
    while IFS= read -r case_name; do
      case_options+=("${case_name}")
    done < <(collect_cases "${LANGUAGE}" "${VARIANT}" "${SHAPE}")
    local case_choice
    case_choice="$(prompt_choice "Select case (all lets the runner run all cases matching current filters)" "${case_options[@]}")"
    if [[ "${case_choice}" != "all" ]]; then
      CASE_NAME="${case_choice}"
    fi
  fi
}

ensure_required_selection() {
  [[ -n "${LANGUAGE}" ]] || die "missing --language, or use interactive mode"
  [[ -n "${VARIANT}" ]] || die "missing --variant, or use interactive mode"
  [[ -n "${SIZE}" ]] || die "missing --size, or use interactive mode"
}

validate_case_selection() {
  if [[ -z "${CASE_NAME}" ]]; then
    return
  fi
  local available_cases=()
  local candidate
  mapfile -t available_cases < <(collect_cases "${LANGUAGE}" "${VARIANT}" "${SHAPE}")
  for candidate in "${available_cases[@]}"; do
    if [[ "${candidate}" == "${CASE_NAME}" ]]; then
      return
    fi
  done
  die "case does not belong to current language/variant/shape: ${CASE_NAME}"
}

verify_local_archive() {
  local archive_path="$1"
  local expected_sha256="$2"
  if [[ -z "${expected_sha256}" ]]; then
    return
  fi
  local actual_sha256
  actual_sha256="$(sha256_file "${archive_path}")"
  if [[ "${actual_sha256}" != "${expected_sha256}" ]]; then
    die "archive hash mismatch: ${archive_path}"
  fi
}

unpack_local_archive() {
  local archive_path="$1"
  local destination_parent="$2"
  require_command tar
  require_command zstd
  log "unpacking dataset: ${archive_path#${REPO_ROOT}/}"
  mkdir -p "${destination_parent}"
  zstd -q -d -f -c "${archive_path}" | tar -xf - -C "${destination_parent}"
}

download_remote_archive() {
  local url="$1"
  local archive_path="$2"
  local expected_sha256="$3"
  local temp_path actual_sha256
  mkdir -p "$(dirname "${archive_path}")"
  temp_path="$(mktemp "${archive_path}.tmp.XXXXXX")"

  if has_command curl; then
    log "downloading dataset archive: ${url}"
    if ! curl -fL --retry 3 --retry-delay 1 -o "${temp_path}" "${url}"; then
      rm -f "${temp_path}"
      die "failed to download dataset archive: ${url}"
    fi
  elif has_command wget; then
    log "downloading dataset archive: ${url}"
    if ! wget -O "${temp_path}" "${url}"; then
      rm -f "${temp_path}"
      die "failed to download dataset archive: ${url}"
    fi
  else
    rm -f "${temp_path}"
    die "missing download tool (curl or wget)"
  fi

  if [[ -n "${expected_sha256}" ]]; then
    actual_sha256="$(sha256_file "${temp_path}")"
    if [[ "${actual_sha256}" != "${expected_sha256}" ]]; then
      rm -f "${temp_path}"
      die "archive hash mismatch: ${url}"
    fi
  fi
  mv "${temp_path}" "${archive_path}"
}

ensure_dataset_ready() {
  local archive_path dir_path sha256 url
  archive_path="$(dataset_archive_path "${VARIANT}" "${SIZE}")"
  dir_path="$(dataset_dir_path "${VARIANT}" "${SIZE}")"
  sha256="${DATASET_SHA256["${VARIANT}/${SIZE}"]:-}"
  url=""
  if [[ -n "${DATASET_BASE_URL}" ]]; then
    url="${DATASET_BASE_URL%/}/datasets/releases/${DATASET_RELEASE}/${VARIANT}/${SIZE}.tar.zst"
  fi

  if [[ -f "${dir_path}/meta.json" ]]; then
    log "using existing dataset directory: $(display_path "${dir_path}")"
    return
  fi

  if [[ -f "${archive_path}" ]]; then
    if [[ -n "${sha256}" ]]; then
      verify_local_archive "${archive_path}" "${sha256}"
    fi
    unpack_local_archive "${archive_path}" "$(dirname "${archive_path}")"
    [[ -f "${dir_path}/meta.json" ]] || die "meta.json still missing after unpack: ${VARIANT}/${SIZE}"
    return
  fi

  if [[ -n "${url}" ]]; then
    download_remote_archive "${url}" "${archive_path}" "${sha256}"
    unpack_local_archive "${archive_path}" "$(dirname "${archive_path}")"
    [[ -f "${dir_path}/meta.json" ]] || die "meta.json still missing after download and unpack: ${VARIANT}/${SIZE}"
    return
  fi

  die "dataset directory or archive not found: ${VARIANT}/${SIZE}"
}

bootstrap_js() {
  require_command node
  require_command npm
  if [[ -f "${BENCHMARK_ROOT}/js/node_modules/tsx/dist/esm/api/index.mjs" \
    && -f "${BENCHMARK_ROOT}/js/node_modules/apache-arrow/Arrow.node.js" \
    && -f "${BENCHMARK_ROOT}/js/node_modules/@ttoon/shared/package.json" \
    && -f "${BENCHMARK_ROOT}/js/node_modules/ttoon-wasm-bridge/package.json" ]]; then
    return
  fi
  log "installing benchmarks/js base dependencies"
  (
    cd "${BENCHMARK_ROOT}/js"
    npm install
  )
  if [[ ! -f "${REPO_ROOT}/rust/crates/wasm-bridge/pkg/package.json" ]]; then
    log "ensuring wasm bridge package is available for JS benchmarks"
    (
      cd "${REPO_ROOT}/js"
      npm run ensure:wasm-bridge
    )
  fi
  log "ensuring JS benchmark shared package is installed"
  (
    cd "${BENCHMARK_ROOT}/js"
    npm install --no-save --no-package-lock ../../js/shared
  )
}

bootstrap_python() {
  require_command uv
  log "ensuring Python benchmark extension is a release build"
  (
    cd "${REPO_ROOT}/python"
    uv run maturin develop --release 1>&2
  )
}

bootstrap_rust() {
  require_command cargo
}

bootstrap_selected_language() {
  if [[ "${NO_BOOTSTRAP}" == "1" ]]; then
    return
  fi
  case "${LANGUAGE}" in
    js) bootstrap_js ;;
    python) bootstrap_python ;;
    rust) bootstrap_rust ;;
    *) die "unknown language: ${LANGUAGE}" ;;
  esac
}

build_case_command() {
  local shape="$1" case_name="$2"
  local cmd=()
  case "${LANGUAGE}" in
    js)
      cmd=(node --max-old-space-size=10240 "benchmarks/js/src/runner.js")
      ;;
    python)
      cmd=("python/.venv/bin/python" "benchmarks/python/runner.py")
      ;;
    rust)
      cmd=(cargo run --release --manifest-path "benchmarks/rust/Cargo.toml" --)
      ;;
    *)
      die "unknown language: ${LANGUAGE}"
      ;;
  esac

  cmd+=(--dataset-root "$(display_path "${PREPARED_ROOT}")")
  cmd+=(--variant "${VARIANT}")
  cmd+=(--size "${SIZE}")
  cmd+=(--shape "${shape}")
  cmd+=(--case "${case_name}")
  cmd+=(--warmups "${WARMUPS}")
  cmd+=(--iterations "${ITERATIONS}")
  cmd+=(--benchmark-release "${BENCHMARK_RELEASE}")
  cmd+=(--dataset-release "${DATASET_RELEASE}")

  if [[ "${TRACE_MEMORY}" == "1" ]]; then
    cmd+=(--trace-memory)
  fi

  printf '%s\n' "${cmd[@]}"
}

print_execution_plan() {
  printf 'language: %s\n' "${LANGUAGE}" >&2
  printf 'variant: %s\n' "${VARIANT}" >&2
  printf 'size: %s\n' "${SIZE}" >&2
  printf 'shape: %s\n' "${SHAPE:-all}" >&2
  printf 'case: %s\n' "${CASE_NAME:-all (per-case subprocess)}" >&2
  printf 'api: %s\n' "${API_MODE}" >&2
  printf 'warmups: %s\n' "${WARMUPS}" >&2
  printf 'iterations: %s\n' "${ITERATIONS}" >&2
  printf 'trace_memory: %s\n' "${TRACE_MEMORY}" >&2
  printf 'benchmark_release: %s\n' "${BENCHMARK_RELEASE}" >&2
  printf 'dataset_release: %s\n' "${DATASET_RELEASE}" >&2
  printf 'dataset_root: %s\n' "${PREPARED_ROOT}" >&2
  printf 'bootstrap: %s\n' "$([[ "${NO_BOOTSTRAP}" == "1" ]] && printf 'disabled' || printf 'enabled')" >&2
}

run_benchmark() {
  local tmp_dir
  tmp_dir="$(mktemp -d)"

  local index=0
  local shape case_name
  for shape in "${SHAPES[@]}"; do
    [[ -n "${SHAPE}" && "${SHAPE}" != "${shape}" ]] && continue
    local entry="${CASE_MATRIX["${LANGUAGE}|${VARIANT}|${shape}"]-}"
    [[ -z "${entry}" ]] && continue
    for case_name in ${entry}; do
      if ! case_matches_api_mode "${case_name}"; then
        continue
      fi
      [[ -n "${CASE_NAME}" && "${CASE_NAME}" != "${case_name}" ]] && continue

      if [[ "${DRY_RUN}" == "1" ]]; then
        mapfile -t cmd_lines < <(build_case_command "${shape}" "${case_name}")
        printf 'dry-run [%s/%s]:\n' "${shape}" "${case_name}" >&2
        printf '  %q' "${cmd_lines[@]}" >&2
        printf '\n' >&2
        index=$((index + 1))
        continue
      fi

      log "running: ${shape}/${case_name}"
      mapfile -t cmd_lines < <(build_case_command "${shape}" "${case_name}")
      local stdout_path="${tmp_dir}/${index}.json"
      local stderr_path="${tmp_dir}/${index}.err"
      (
        cd "${REPO_ROOT}"
        "${cmd_lines[@]}"
      ) > "${stdout_path}" 2> "${stderr_path}" || {
        log "warning: ${shape}/${case_name} failed"
        if [[ -s "${stderr_path}" ]]; then
          sed 's/^/[bench] stderr: /' "${stderr_path}" >&2
        fi
      }
      index=$((index + 1))
    done
  done

  if [[ "${DRY_RUN}" == "1" ]]; then
    return
  fi

  if (( index == 0 )); then
    die "no cases to run"
  fi

  python3 -c '
import json, sys, os
tmp = sys.argv[1]
base = None
results = []
issues = []
for name in sorted(os.listdir(tmp)):
    if not name.endswith(".json"):
        continue
    fpath = os.path.join(tmp, name)
    try:
        with open(fpath) as f:
            p = json.load(f)
    except (json.JSONDecodeError, OSError):
        continue
    if base is None:
        base = p
    results.extend(p.get("results", []))
    issues.extend(p.get("issues", []))
if base:
    base["results"] = results
    base["issues"] = issues
    print(json.dumps(base, ensure_ascii=False, indent=2))
' "${tmp_dir}"

  rm -rf "${tmp_dir}"
}

parse_args() {
  while (($# > 0)); do
    case "$1" in
      --language)
        LANGUAGE="${2:?missing value for --language}"
        shift 2
        ;;
      --variant)
        VARIANT="${2:?missing value for --variant}"
        shift 2
        ;;
      --size)
        SIZE="${2:?missing value for --size}"
        shift 2
        ;;
      --shape)
        SHAPE="${2:?missing value for --shape}"
        shift 2
        ;;
      --case)
        CASE_NAME="${2:?missing value for --case}"
        shift 2
        ;;
      --api)
        API_MODE="${2:?missing value for --api}"
        shift 2
        ;;
      --warmups)
        WARMUPS="${2:?missing value for --warmups}"
        shift 2
        ;;
      --iterations)
        ITERATIONS="${2:?missing value for --iterations}"
        shift 2
        ;;
      --dataset-root)
        PREPARED_ROOT="${2:?missing value for --dataset-root}"
        shift 2
        ;;
      --interactive)
        FORCE_INTERACTIVE="1"
        shift
        ;;
      --non-interactive)
        FORCE_INTERACTIVE="0"
        shift
        ;;
      --list-datasets)
        LIST_DATASETS="1"
        shift
        ;;
      --list-cases)
        LIST_CASES="1"
        shift
        ;;
      --trace-memory)
        TRACE_MEMORY="1"
        shift
        ;;
      --dry-run)
        DRY_RUN="1"
        shift
        ;;
      --yes)
        YES="1"
        shift
        ;;
      --no-bootstrap)
        NO_BOOTSTRAP="1"
        shift
        ;;
      -h|--help)
        usage
        exit 0
        ;;
      *)
        die "unknown argument: $1"
        ;;
    esac
  done
}

main() {
  parse_args "$@"
  validate_release_manifests
  validate_args

  if [[ "${LIST_DATASETS}" == "1" ]]; then
    list_datasets
    exit 0
  fi

  if [[ "${LIST_CASES}" == "1" ]]; then
    print_case_matrix "${LANGUAGE}" "${VARIANT}" "${SHAPE}"
    exit 0
  fi

  resolve_prompted_args
  ensure_required_selection
  validate_args
  validate_case_selection

  print_execution_plan
  confirm_continue || die "user cancelled"

  if [[ "${DRY_RUN}" != "1" ]]; then
    bootstrap_selected_language
    ensure_dataset_ready
  fi

  run_benchmark
}

main "$@"
