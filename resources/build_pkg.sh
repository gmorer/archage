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
    # chown -R root:root . $CCACHE_DIR /build/srcs
    chown -R root:root /build
  }
  local pkg=$1

  trap cleanup EXIT

  source PKGBUILD
  yes | pacman -Syu --cachedir /build/cache/pacman --noconfirm ${depends[@]} ${makedepends[@]} ccache mold ||

  # Getting "Operation not permitted" otherwise
  set_perm "/usr/sbin"

  chown -R builder:builder . $CCACHE_DIR /build/srcs
  rm -rf /build/srcs/$pkg


  # TODO:
  # get sources:
  # BUILDDIR=/tmp/srcs makepkg -f -c --skippgpcheck  --nobuild -o
  # BUILDDIR=/tmp/srcs PKGDEST=/tmp/pkg makepkg -f --skippgpcheck -e
 
  # Skip pgp because it fail on the ccache pkg.
  runuser -u builder -m -- makepkg -f -c --skippgpcheck --config /build/makepkg.conf --nobuild
  # chown -R root:root /build/srcs
  # chown -R builder:builder /build/srcs/$pkg
  pkgdest=$(mktemp -d)
  chown -R builder:builder $pkgdest
  PKGDEST=$pkgdest runuser -u builder -m -- makepkg -f --skippgpcheck --config /build/makepkg.conf --noextract -c
  # chown -R root:root $pkgdest
  mv $pkgdest/* /build/repo
  runuser -u builder -- makepkg --printsrcinfo > .SRCINFO
  rm -rf /build/srcs/$pkg/pkg
  ccache -s
)

export BUILDDIR=/build/srcs

for pkg in $@; do
  pushd pkgs/$pkg
  build_pkg $pkg
  popd
done
