#!/usr/bin/env bash
set -eufo pipefail

# A script to compile the rust target compatible with
# https://github.com/cli/gh-extension-precompile

ext=""
if [[ "${OSTYPE}" == "msys" ]]; then
  ext=".exe"
fi

rustup target add ${CARGO_BUILD_TARGET:-}

if [[ "${CARGO_BUILD_TARGET:-}" == *"android"* ]]; then
  underscore_target=$(echo "${CARGO_BUILD_TARGET}" | tr '-' '_')
  UNDERSCORE_TARGET=$(echo "${underscore_target}" | tr '[:lower:]' '[:upper:]')
  export CC_${underscore_target}=/usr/local/lib/android/sdk/ndk/26.3.11579264/toolchains/llvm/prebuilt/linux-x86_64/bin/${CARGO_BUILD_TARGET}21-clang
  export AR=/usr/local/lib/android/sdk/ndk/26.3.11579264/toolchains/llvm/prebuilt/linux-x86_64/bin/llvm-ar
  export RANLIB=/usr/local/lib/android/sdk/ndk/26.3.11579264/toolchains/llvm/prebuilt/linux-x86_64/bin/llvm-ranlib
  export CARGO_TARGET_${UNDERSCORE_TARGET}_LINKER=/usr/local/lib/android/sdk/ndk/26.3.11579264/toolchains/llvm/prebuilt/linux-x86_64/bin/${CARGO_BUILD_TARGET}21-clang
fi

cargo build --release && mkdir dist && cp target/${CARGO_BUILD_TARGET}/release/gh-news"$ext" dist/gh-news"$1"_"${TARGET}""$ext"
