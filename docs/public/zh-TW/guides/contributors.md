---
title: 貢獻者指南
sidebar_position: 9
sidebar_label: 貢獻者
description: 維護者與文件貢獻者的入口，包含公開文件來源、同步流程與發布準備。
---

# 貢獻者指南

本頁面面向維護者、文件貢獻者，以及需要準備公開發布的開發者。

## 文件來源與同步規則

- `docs/public/` 是公開文件的權威來源
- `public/docs/public/` 是同步後輸出，不應手動編輯
- 日常開發請在 private repo 進行，並使用一般工作分支，例如 `feat/...`、`fix/...`、`docs/...`、`chore/...`
- `main` 應保留給已整理好、可進入 release 流程的內容
- public repo 不作日常開發；它只接收從 private 同步後、準備公開發布的結果
- release 前若有公開文件變更，請在 repo root 執行：

```bash
bash op/sync_public.sh
```

## CI / Release 原則

- private workflow：手動觸發，可執行測試、編譯與本地打包檢查
- public workflow：不跑測試，只做編譯、打包與發布產物輸出
- registry 發布目前獨立於 public workflow 之外；workflow 本身負責產出發布產物，維護者則在 release 流程中另外發布到 PyPI、npm 與 crates.io

## 套件與發布產物

公開發布目前會產出三類產物：

- Python：wheel 與 sdist
- JavaScript：`@ttoon/shared`、`@ttoon/node`、`@ttoon/web` 的 `.tgz`
- Rust：`ttoon-core` 的 `.crate`

目前已發布的套件為 PyPI 的 `ttoon`、npm 的 `@ttoon/shared` / `@ttoon/node` / `@ttoon/web`，以及 crates.io 的 `ttoon-core`。實際安裝方式請參考 [安裝](../getting-started/installation.md)。

## Benchmark 與 dataset

benchmark suite 與 dataset release 由 manifest 鎖定：

- `benchmarks/manifests/benchmark_release.sh`
- `benchmarks/manifests/datasets.sh`

benchmark dataset 目前存放於 R2，執行 benchmark 時，若本地缺少 archive，shell 與 Python 入口會自動下載、驗 SHA256 並解壓。詳細流程請參考 [Benchmark 指南](benchmarks.md)。
