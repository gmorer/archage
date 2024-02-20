#!/usr/bin/env bash

# This script meant to be used inside a container
# USAGE: <script> <pkgs...>

set -e

build_pkg() {
  source PKGBUILD
  pacman -Sy --noconfirm ${depends[@]} ${makedepends[@]}
  chmod -R a+w .
  chmod -R a+r .
  runuser -unobody -m -- makepkg -c -f --config /build/makepkg.conf
  runuser -unobody -- makepkg --printsrcinfo > .SRCINFO
  chown -R root:root *
}


for pkg in $@; do
  pushd pkgs/$pkg
  build_pkg $pkgbuild
  popd
done
