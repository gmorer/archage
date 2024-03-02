#!/usr/bin/env bash

# This script meant to be used inside a container
# USAGE: <script> <pkgs...>

set -e
set -x

# Adding an builder user
if ! id builder; then
  useradd -M builder
  # pacman-key --refresh-keys
fi

# TODO: set back ownership on failure

# Remove capabilities of files in a directory $1
set_perm() {
  local dir=$1
  getcap $dir/* | cut -d' ' -f1 | while read line ; do setcap -r $line ; done
}

build_pkg() (
  function cleanup() {
    echo "Cleaning up build directory"
    chown -R root:root . $CCACHE_DIR
  }

  trap cleanup EXIT

  source PKGBUILD
  yes | pacman -Syu --noconfirm ${depends[@]} ${makedepends[@]} ccache ||

  # Getting "Operation not permitted" otherwise
  set_perm "/usr/sbin"

  chown -R builder:builder . $CCACHE_DIR
  # Skip pgp because it fail on the ccache pkg.
  runuser -u builder -m -- makepkg -c -f --skippgpcheck --config /build/makepkg.conf
  runuser -u builder -- makepkg --printsrcinfo > .SRCINFO
  ccache -s
)

for pkg in $@; do
  pushd pkgs/$pkg
  build_pkg $pkgbuild
  popd
done
