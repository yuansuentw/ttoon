---
title: Benchmark 指南
sidebar_position: 8
sidebar_label: Benchmark
description: benchmark suite、dataset lifecycle、R2 自動下載與常用執行方式。
---

# Benchmark 指南

TTOON 的 benchmark suite 用於比較 Python、JavaScript、Rust 三個 SDK 在 object path 與 Arrow path 上的序列化、反序列化與串流行為。

## 目錄與權威 manifest

- benchmark 入口：`benchmarks/bench.sh`
- Python runner：`benchmarks/python/runner.py`
- 全量執行：`benchmarks/scripts/run_all.py`
- benchmark release pin：`benchmarks/manifests/benchmark_release.sh`
- dataset release 與 SHA256 pin：`benchmarks/manifests/datasets.sh`

## dataset 準備流程

預設 dataset root 為 `benchmarks/datasets/prepared`。

當 benchmark 入口發現 `prepared/<variant>/<size>/meta.json` 不存在時，會依序執行：

1. 若本地已有 `prepared/<variant>/<size>.tar.zst`，先驗 SHA256，再解壓
2. 若本地 archive 不存在，則從 `DATASET_BASE_URL` 下載
3. 下載後再驗證 `benchmarks/manifests/datasets.sh` 內的 SHA256
4. 解壓到 `prepared/<variant>/<size>/`

也就是說，現在 benchmark 可以直接從 R2 補抓缺少的 dataset，不必先手動 provision。

## 常用指令

### 列出 dataset

```bash
./benchmarks/bench.sh --list-datasets
```

### 列出 case

```bash
./benchmarks/bench.sh --language js --variant js-basic --shape structure --list-cases
```

### 跑單一 benchmark

```bash
./benchmarks/bench.sh --language js --variant js-basic --size 10k --shape structure
./benchmarks/bench.sh --language python --variant js-basic --size 100k --shape tabular --api streaming
./benchmarks/bench.sh --language rust --variant extended --size 100k --case arrow_ttoon_deserialize
```

### 全量執行並輸出 summary / report

```bash
uv run --project python python benchmarks/scripts/run_all.py
```

## 語言別注意事項

### Python

- `benchmarks/bench.sh` 會自動確保 Python extension 為 release build
- 若直接執行 `benchmarks/python/runner.py`，仍建議先跑：

```bash
cd python
uv run maturin develop --release
```

### JavaScript

- JS benchmark 需要 `rust/crates/wasm-bridge/pkg`
- 若 `pkg/` 缺失且本地有 `wasm-pack`，相關腳本會自動補建
- 若 `pkg/` 缺失且沒有 `wasm-pack`，JS benchmark 會 fail-fast

### Rust

- Rust benchmark 直接使用 workspace 內的 release build 路徑

## 結果輸出

`run_all.py` 會在 `benchmarks/results/<timestamp>/` 產出：

- `raw/*.json`
- `summary.json`
- `summary.csv`
- `report.md`

這些結果都會內嵌 `benchmark_release` 與 `dataset_release`，用來對齊報表、dataset 與 release note。

## 只讀現有資料，不自動下載或解壓

Python 入口可使用：

```bash
uv run --project python python benchmarks/scripts/run_all.py --no-auto-unpack
uv run --project python python benchmarks/python/runner.py --no-auto-unpack
```

這模式只讀現有 extracted datasets，不會下載也不會解壓 archive。
