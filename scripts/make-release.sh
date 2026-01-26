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

generate_release_note() {
  git log v${CARGO_VERSION}..HEAD | aichat "Generate release notes for version ${VERSION} from version ${CARGO_VERSION} in markdown,
  this will be used for Github release. Categorize changes into Features, Fixes, Misc. If there sib reaking changes, highlight them at the top.
  If there is no changes then don't say there is no changes provided."
}

bumpversion() {
  local current major minor patch mode
  current=$(git describe --tags $(git rev-list --tags --max-count=1) || echo 0.0.0)
  current=${current#v}
  echo "Current tag version is ${current}"

  major=$(uv run --with semver python3 -c "import semver,sys;print(str(semver.VersionInfo.parse(sys.argv[1]).bump_major()))" ${current})
  minor=$(uv run --with semver python3 -c "import semver,sys;print(str(semver.VersionInfo.parse(sys.argv[1]).bump_minor()))" ${current})
  patch=$(uv run --with semver python3 -c "import semver,sys;print(str(semver.VersionInfo.parse(sys.argv[1]).bump_patch()))" ${current})

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
RELEASE_NOTE="$(generate_release_note)"
git commit -S -m "Release ${VERSION} 🥳" -m "${RELEASE_NOTE}" ${vfile} Cargo.lock || true
[[ ${VERSION} != v* ]] && VERSION="v${VERSION}"
git tag -s ${VERSION} -m "${RELEASE_NOTE}"
git push --tags origin ${VERSION}
git push origin main --no-verify
[[ -n ${NO_PUBLISH:-""} ]] && exit
env CARGO_REGISTRY_TOKEN=$(pass show cargo/token) cargo publish
