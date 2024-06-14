#!/usr/bin/env bash

# This script meant to be used inside a container
# USAGE: <script> <pkgs...>

set -eE
set -x


# For makepkg sources outputs
export BUILDDIR=/build/srcs

function cleanup() {
  # echo "Cleaning up build directory"
  rm -rf /build/srcs/*/pkg
  # chown -R root:root . $CCACHE_DIR /build/srcs
  chown -R root:root /build
}
trap cleanup EXIT ERR

pacman_install() {
  yes | pacman -S --cachedir /build/cache/pacman --noconfirm $@

  # Remove capabilities of files
  getcap /usr/sbin | cut -d' ' -f1 | while read line ; do setcap -r $line ; done
}

init_system() {
# Adding an builder user
  if ! id builder; then
    yes | pacman -Syu --cachedir /build/cache/pacman --noconfirm git ccache mold $@
    useradd -U -M builder
    # pacman-key --refresh-keys
  fi
}

pacage_build() (
  local pkg=$1
  source PKGBUILD
  # Check if variable is defined
  pacman_install ${depends[@]} ${makedepends[@]} ${checkdepends[@]}
  chown -R builder:builder . $CCACHE_DIR /build/srcs

  local pkgdest=$(mktemp -d)
  chown -R builder:builder $pkgdest
  PKGDEST=$pkgdest runuser -u builder -m -- makepkg -f --skippgpcheck --config /build/makepkg.conf --noextract
  mv $pkgdest/* /build/repo
  runuser -u builder -- makepkg --printsrcinfo > .SRCINFO
  ccache -s
)

pacage_get() (
  local pkg=$1
  source PKGBUILD
  # pacman_install ${depends[@]} ${makedepends[@]}
  chown -R builder:builder . $CCACHE_DIR /build/srcs

  # To test makepkg --allsource

  # we remove old sources
  rm -rf /build/srcs/$pkg
  runuser -u builder -m -- makepkg -f -c --nodeps --nocheck --skippgpcheck --config /build/makepkg.conf --nobuild
  # maybe it should print some info in the src dir
)

action=$1
set -- "${@:2}" 
pkgs=$@

# we remove [0] which is the action
echo action $action
echo pkgs $pkgs

case $action in
  "start")
    init_system
  ;;
  "get")
    for pkg in "${pkgs[@]}"; do
      pushd pkgs/$pkg
      pacage_get $pkg
      popd
    done
  ;;
  "build")
    for pkg in "${pkgs[@]}"; do
      pushd pkgs/$pkg
      pacage_build $pkg
      popd
    done
  ;;
  *)
    "Invalid action: $action"
    exit 2
  ;;
esac
exit 0


  # TODO:
  # get sources:
  # BUILDDIR=/tmp/srcs makepkg -f -c --skippgpcheck  --nobuild -o
  # BUILDDIR=/tmp/srcs PKGDEST=/tmp/pkg makepkg -f --skippgpcheck -e

 
  # Skip pgp because it fail on the ccache pkg.
  # chown -R root:root /build/srcs
  # chown -R builder:builder /build/srcs/$pkg

