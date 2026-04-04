---
title: Contributors Guide
sidebar_position: 9
sidebar_label: Contributors
description: Entry point for maintainers and doc contributors, including public doc sources, sync flow, and release prep.
---

# Contributors Guide

This page is for maintainers, doc contributors, and developers preparing a public release.

## Doc Source and Sync Rules

- `docs/public/` is the authoritative source for public docs
- `public/docs/public/` is synced output and should not be edited manually
- Do day-to-day development in the private repo, using normal working branches such as `feat/...`, `fix/...`, `docs/...`, or `chore/...`
- Keep `main` for content that is already cleaned up and ready to enter the release flow
- Do not use the public repo for everyday development; it should only receive synced, release-ready public content from the private repo
- Before a release that changes public docs, run this from the repo root:

```bash
bash op/sync_public.sh
```

## CI / Release Policy

- private workflow: manual trigger, with tests, build, and local package checks
- public workflow: no tests, build and package only, plus release artifacts
- registry publication is currently handled separately from the public workflow; the workflow itself builds release artifacts, while maintainers publish to PyPI, npm, and crates.io as part of the release process

## Package and Release Artifacts

The public release currently produces three artifact groups:

- Python: wheels and sdist
- JavaScript: `.tgz` packages for `@ttoon/shared`, `@ttoon/node`, and `@ttoon/web`
- Rust: `.crate` for `ttoon-core`

Published packages are available on PyPI (`ttoon`), npm (`@ttoon/shared`, `@ttoon/node`, `@ttoon/web`), and crates.io (`ttoon-core`). See [Installation](../getting-started/installation.md) for the public install commands.

## Benchmark and Dataset

The benchmark suite and dataset release are pinned by manifest files:

- `benchmarks/manifests/benchmark_release.sh`
- `benchmarks/manifests/datasets.sh`

Benchmark datasets are currently stored in R2. When a local archive is missing, the shell and Python entry points automatically download it, verify the SHA256 pin, and unpack it. See the [Benchmark Guide](benchmarks.md) for details.
