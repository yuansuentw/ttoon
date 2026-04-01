---
title: 貢獻者指南
sidebar_position: 9
sidebar_label: 貢獻者
description: 維護者與文件貢獻者的入口，包含 public docs 來源、同步流程與 release 準備。
---

# 貢獻者指南

本頁面面向維護者、文件貢獻者，以及需要準備 public release 的開發者。

## 文件來源與同步規則

- `docs/public/` 是 public 文件的權威來源
- `public/docs/public/` 是同步後輸出，不應手動編輯
- 日常開發請在 private repo 進行，並使用一般工作分支，例如 `feat/...`、`fix/...`、`docs/...`、`chore/...`
- `main` 應保留給已整理好、可進入 release 流程的內容
- public repo 不作日常開發；它只接收從 private 同步後、準備公開發布的結果
- release 前若有 public 文件變更，請在 repo root 執行：

```bash
bash op/sync_public.sh
```

## CI / Release 原則

- private workflow：手動觸發，可執行測試、編譯與本地打包檢查
- public workflow：不跑測試，只做編譯、打包與 release artifact 產出
- public release 不發布到 PyPI、npm、crates.io；僅輸出可本地安裝的 artifacts

## 套件與 release artifact

public release 目前會產出三類 artifacts：

- Python：wheel 與 sdist
- JavaScript：`@ttoon/shared`、`@ttoon/node`、`@ttoon/web` 的 `.tgz`
- Rust：`ttoon-core` 的 `.crate`

實際安裝方式請參考 [安裝](../getting-started/installation.md)。

## Benchmark 與 dataset

benchmark suite 與 dataset release 由 manifest 鎖定：

- `benchmarks/manifests/benchmark_release.sh`
- `benchmarks/manifests/datasets.sh`

benchmark dataset 目前存放於 R2，執行 benchmark 時，若本地缺少 archive，shell 與 Python 入口會自動下載、驗 SHA256 並解壓。詳細流程請參考 [Benchmark 指南](benchmarks.md)。
