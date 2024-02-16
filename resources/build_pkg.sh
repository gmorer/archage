#!/usr/bin/env bash

# This script meant to be used inside a container

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

chmod -R a+x /build
pkgs=$(ls */PKGBUILD | cut -d '/' -f1)

for i in $pkgs; do
  cd $i
  build_pkg
  cd ..
done
