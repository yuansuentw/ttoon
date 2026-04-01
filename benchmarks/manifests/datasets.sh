#!/usr/bin/env bash

# shellcheck shell=bash
#
# Benchmark dataset manifest.
# Paths follow fixed convention: {prepared_root}/{variant}/{size}[.tar.zst]
# Only SHA256 hashes need explicit declaration; everything else is derived.

DATASET_MANIFEST_VERSION="1"
DATASET_RELEASE="1"
DATASET_STORAGE_PROVIDER="r2"
DATASET_BASE_URL="https://ttoon.dev"

declare -A DATASET_SHA256=(
  ["js-basic/10k"]="b37e7dba491d0890bf370296a34949a2afddf6893d07d704c31a599d901465cd"
  ["js-basic/100k"]="a3aceeb67a49086f7b52a580cf6560bb0c643bf3837d2a0c50aa2b9b2639f730"
  ["js-basic/1m"]="edeed69717977ffcb5f24448b7126f342c0a268c2f40cf4b4f76cd2cf098e4d5"
  ["extended/10k"]="446ba170c849608aa80dc25ce93bbcd6c8dea36a4c9a65a70463b1947a5b7997"
  ["extended/100k"]="f094afee2349332c5b9727d1b6defda04365abf7058662b6324adec8806cc76e"
  ["extended/1m"]="8f8dff323c55faade94e73bbf1f12100949ba600bc7894e42911d367da36afdf"
)
