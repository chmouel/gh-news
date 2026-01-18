#!/usr/bin/env bash
set -eufo pipefail

# A script to compile the rust target compatible with
# https://github.com/cli/gh-extension-precompile

ext=""
if [[ "${OSTYPE}" == "msys" ]]; then
  ext=".exe"
fi

rustup target add ${CARGO_BUILD_TARGET:-}
cargo build --release && mkdir -p dist &&
  cp target/${CARGO_BUILD_TARGET}/release/gh-news"$ext" dist/gh-news"$1"_"${TARGET}""$ext"
