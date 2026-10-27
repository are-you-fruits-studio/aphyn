#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$repo_root"

target_doc_dir="target/doc"
target_main_crate_index="$target_doc_dir/aphyn/index.html"

if [[ "${SKIP_RUSTDOC:-0}" != "1" ]]; then
  if [[ "${CI:-}" == "true" ]] && command -v apt-get >/dev/null 2>&1 && ! pkg-config --exists alsa; then
    sudo apt-get update
    sudo apt-get install -y pkg-config libasound2-dev libudev-dev
  fi

  cargo +nightly doc --workspace --no-deps
elif [[ ! -f "$target_main_crate_index" ]]; then
  echo "Cannot skip rustdoc generation because target/doc/aphyn/index.html does not exist. Run with SKIP_RUSTDOC unset once." >&2
  exit 1
fi

if [[ ! -d "$target_doc_dir" ]]; then
  echo "Rustdoc output was not found at target/doc. Run cargo doc first." >&2
  exit 1
fi

if [[ ! -f "$target_main_crate_index" ]]; then
  echo "Main rustdoc entry was not found: $target_main_crate_index" >&2
  exit 1
fi

echo "Rustdoc API reference generated at: $target_doc_dir"
