#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$repo_root"

target_doc_dir="target/doc"
build_api_dir="docs/build/api-reference"
main_crate_index="$target_doc_dir/aphyn/index.html"
build_main_crate_index="$build_api_dir/aphyn/index.html"

if [[ ! -f "$main_crate_index" ]]; then
  echo "Main rustdoc entry was not found: $main_crate_index" >&2
  exit 1
fi

# rustdoc output is fully self-contained with relative links (../static.files/, data-root-path="../").
# Served as a static subtree under the Docusaurus baseUrl, those relative paths resolve correctly,
# so we copy the tree verbatim with no path rewriting.
rm -rf "$build_api_dir"
mkdir -p "$build_api_dir"
cp -a "$target_doc_dir/." "$build_api_dir/"

if [[ ! -f "$build_main_crate_index" ]]; then
  echo "Failed to copy rustdoc into Docusaurus build: $build_main_crate_index" >&2
  exit 1
fi

echo "Rustdoc API reference copied to: $build_api_dir"
