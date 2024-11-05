#!/usr/bin/env bash

# This script meant to be used inside a container
# USAGE: <script> <pkgs...>

set -eE
set -x


# For makepkg sources outputs
export BUILDDIR=/build/srcs
export PATH=$PATH:/usr/bin/vendor_perl


action=$1
pkg=$2
# set -- "${@:2}" 

function cleanup() {
  # echo "Cleaning up build directory"
  rm -rf /build/srcs/*/pkg
  # chown -R root:root . $CCACHE_DIR /build/srcs
  chown -R root:root /build/srcs/$pkg /build/pkgs/$pkg
}
trap cleanup EXIT ERR

pacman_install() {
  yes | pacman -S --cachedir /build/cache/pacman --noconfirm $@

  # Remove capabilities of files
  getcap /usr/sbin | cut -d' ' -f1 | while read line ; do setcap -r $line ; done
}

init_system() {
# Adding an builder user
  if ! command -v ccache ; then 
  # if ! id builder; then
    yes | pacman -Syu --cachedir /build/cache/pacman --noconfirm git ccache mold glibc-locales
    # useradd -U -M builder

    # Local stuff
    localedef -c -f UTF-8 -i en_US en_US.UTF-8
    export LC_ALL=en_US.UTF-8
    export LANG=en_US.UTF-8
    echo "LANG=en_US.UTF-8" > /etc/locale.conf

    # pacman-key --refresh-keys
  fi
}

pacage_build() (
  env
  local pkg=$1
  local usr=$pkg"_builder"
  local makepkg_conf="/build/srcs/makepkg_${pkg}.conf"
  if [ ! -d /build/srcs/$pkg ] ; then
    echo "$pkg source dir is missing";
    false
  fi

  if ! id $usr ; then
    useradd -U -M $usr
  fi
  source PKGBUILD

  # Check if variable is defined
  pacman_install ${depends[@]} ${makedepends[@]} ${checkdepends[@]}

  git --version

  local pkgdest=$(mktemp -d)
  chown -R ${usr}:${usr} . $pkgdest $makepkg_conf $CCACHE_DIR /build/srcs/$pkg
  PKGDEST=$pkgdest runuser -u $usr -m -- makepkg -f --skippgpcheck --skipinteg --config $makepkg_conf --noextract
  mv $pkgdest/* /build/repo
  runuser -u $usr -- makepkg --printsrcinfo > .SRCINFO
  ccache -s
)

pacage_get() (
  local pkg=$1
  local usr=$pkg"_builder"
  local makepkg_conf="/build/srcs/makepkg_${pkg}.conf"
  if ! id $usr ; then
    useradd -U -M $usr
  fi
  pwd
  ls 
  source PKGBUILD

  # rm -rf /build/srcs/$pkg
  # mkdir /build/srcs/$pkg
  # chown -R ${usr}:${usr} . $pkgdest $makepkg_conf $CCACHE_DIR /build/srcs/$pkg
  chown -R ${usr}:${usr} . $makepkg_conf
  chmod o+wx /build/srcs

  # To test makepkg --allsource

  # we remove old sources
  # rm -rf /build/srcs/$pkg
  ls -l /build
  ls -l /build/srcs
  ls -ld /build/srcs
  runuser -u $usr -m -- mkdir /build/srcs/$pkg
  runuser -u $usr -m -- mkdir /build/srcs/$pkg/src
  runuser -u $usr -m -- chmod a-s /build/srcs/$pkg
  ls -ld /build/srcs/$pkg
  ls -ld /build/srcs/$pkg/src
  # runuser -u $usr -m -- bash -x makepkg -f -c --nodeps --nocheck --skippgpcheck --skipinteg --config $makepkg_conf --nobuild
  runuser -u $usr -m -- makepkg -f -c --nodeps --nocheck --skippgpcheck --skipinteg --config $makepkg_conf --nobuild
  # maybe it should print some info in the src dir
)

# we remove [0] which is the action
echo action $action
echo pkg $pkg

case $action in
  "start")
    init_system
  ;;
  "get")
    pushd pkgs/$pkg
    pacage_get $pkg
    popd
  ;;
  "build")
    pushd pkgs/$pkg
    pacage_build $pkg
    popd
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

