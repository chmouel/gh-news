#!/usr/bin/env bash
set -euf
VERSION=${1-""}
CARGO_VERSION=$(grep '^version = "' Cargo.toml | grep -Eo '[0-9]+\.[0-9]+\.[0-9]+')
PKGNAME=$(grep '^name = "' Cargo.toml | sed -E 's/.*"([^"]*)"/\1/')
TMP_RLNOTE=$(mktemp /tmp/.mm.XXXXXX)
clean() { rm -f ${TMP_RLNOTE}; }
trap clean EXIT

# Make sure we are clean git state
[[ -z ${FORCE:-} && -n $(git status --porcelain) ]] && {
  echo "you have uncommitted changes, please commit or stash them first"
  exit 1
}

bumpversion() {
  local current major minor patch mode
  current=$(git describe --tags $(git rev-list --tags --max-count=1) || echo 0.0.0)
  current=${current#v}
  major=$(uv run --with semver python3 -c "import semver,sys;print(str(semver.VersionInfo.parse(sys.argv[1]).bump_major()))" ${current})
  minor=$(uv run --with semver python3 -c "import semver,sys;print(str(semver.VersionInfo.parse(sys.argv[1]).bump_minor()))" ${current})
  patch=$(uv run --with semver python3 -c "import semver,sys;print(str(semver.VersionInfo.parse(sys.argv[1]).bump_patch()))" ${current})

  echo "Change from ${current} to HEAD"
  git log $(git tag | tail -1)..HEAD --pretty=format:"- %s"
  echo "If we bump we get, Major: ${major} Minor: ${minor} Patch: ${patch}"
  read -p "To which version you would like to bump [M]ajor, Mi[n]or, [P]atch or Manua[l]: " ANSWER
  if [[ ${ANSWER,,} == "m" ]]; then
    mode="major"
  elif [[ ${ANSWER,,} == "n" ]]; then
    mode="minor"
  elif [[ ${ANSWER,,} == "p" ]]; then
    mode="patch"
  elif [[ ${ANSWER,,} == "l" ]]; then
    read -p "Enter version: " -e VERSION
    return
  else
    echo "no or bad reply??"
    exit
  fi
  VERSION=$(uv run --with semver python3 -c "import semver,sys;print(str(semver.VersionInfo.parse(sys.argv[1]).bump_${mode}()))" ${current})
  [[ -z ${VERSION} ]] && {
    echo "could not bump version automatically"
    exit
  }
  echo "[release] Releasing ${VERSION}"
}

[[ $(git rev-parse --abbrev-ref HEAD) != main ]] && {
  echo "you need to be on the main branch"
  exit 1
}
[[ -z ${VERSION} ]] && bumpversion

vfile=Cargo.toml
sed -i "s/^version = .*/version = \"${VERSION}\"/" ${vfile}
cargo build --release
git commit -S -m "Release ${VERSION} 🥳" ${vfile} Cargo.lock || true
[[ ${VERSION} != v* ]] && VERSION="v${VERSION}"
git tag -s ${VERSION} -m "Release ${VERSION} 🥳"
git push --tags origin ${VERSION}
git push origin main --no-verify
[[ -n ${NO_PUBLISH:-""} ]] && exit
env CARGO_REGISTRY_TOKEN=$(pass show cargo/token) cargo publish
