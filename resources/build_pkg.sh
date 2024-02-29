#!/usr/bin/env bash

# This script meant to be used inside a container
# USAGE: <script> <pkgs...>

set -e
set -x


# Adding an builder user
if ! id builder; then
  useradd -M builder
fi

# Remove capabilities of files in a directory $1
set_perm() {
  local dir=$1
  getcap $dir/* | cut -d' ' -f1 | while read line ; do setcap -r $line ; done
}

build_pkg() {
  source PKGBUILD
  pacman -Sy --noconfirm ${depends[@]} ${makedepends[@]}

  # Getting "Operation not permitted" otherwise
  set_perm "/usr/sbin"

  chown -R builder:builder *
  runuser -u builder -m -- makepkg -c -f --config /build/makepkg.conf
  runuser -u builder -- makepkg --printsrcinfo > .SRCINFO
  chown -R root:root *
}

for pkg in $@; do
  pushd pkgs/$pkg
  build_pkg $pkgbuild
  popd
done
