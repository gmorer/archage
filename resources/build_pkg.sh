#!/usr/bin/env bash

# This script meant to be used inside a container
# USAGE: <script> <pkgs...>

set -e

build_pkg() {
  env
  source PKGBUILD
  pacman -Sy --noconfirm ${depends[@]} ${makedepends[@]}
  chmod -R a+w .
  chmod -R a+r .
  runuser -unobody -m -- makepkg -c -f
  runuser -unobody -- makepkg --printsrcinfo > .SRCINFO
  chown -R root:root *
}

for i in $1; do
  cd $i
  build_pkg
  cd ..
done
