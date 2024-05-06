# Pacage
Selectively locally compile arch packages for more customisation. Kindof like geento style (pacman+portage). Once the packages are build, install then the normal way with pacman

**No need for root**.

## Usage
Add a new entry to the `/etc/pacman.conf` with the output path of *pacage*, should be before `[core]` and `[extra]`.
**/etc/pacman.conf**:
```
[...]

[pacage]
SigLevel = Optional TrustAll
Server = file://<SERVER_DIR>/repo

[...]
```
## Configuration

### CLI interface
```bash
# Download and build a package
$> cabage get <pkg_name>

# Download pkg sources
$> cabage download <pkg_name>

# (re)build a pkg
$> cabage build <pkg_name>

# list built packages
$> cabage list

# Download latest for every build packages and build them
$> cabage update (<pkg_name>)
```

### Conf file
```toml
container_runner = "podman"         # could be docker, podman-remote ...
server_dir = "/pacage"              # which directory it will operate in, download packages, pacman database...
host_server_dir = "/volumes/pacage" # Optional, real server_dir location, if running inside a container and using podman-remote for example, default: <server_dir>
build_log_dir = "/pacage/log"       # default: none

# man 5 makepkg.conf
[makepkg]
packager = "user <user@local.localhost>"
cflags = "-march=native -O2 --param=l1-cache-size=32 --param=l2-cache-size=512"
cxxflags = "-march=native -O2 --param=l1-cache-size=32 --param=l2-cache-size=512"
ltoflags = "-flto=auto"
ccache = true # Replace BUILD_ENV with BUILDENV=(!distcc color ccache check !sign), default: false

# List of the packages to compile
[vi]
[vi.makepkg]
ccache = false

[linux]

```

## Server dir
```bash
├ pkgs/                # Packages pkgbuild
│ ├ some_package/
│ └ [..]
│
├ cache/
│ ├ ccache/             # ccache dir
│ └ pacman/
│
├ srcs/                 # package source dir
│ ├ some_package/
│ └ [..]
│
├ repo/
│ ├ some_package/
│ ├ pacage_build.sh
│ ├ pacage.db@ -> pacage.db.tar.gz
│ ├ pacage.db.tar.gz
│ ├ pacage.files@ -> pacage.files.tar.gz
│ ├ pacage.files.tar.gz
│ ├ some_package-0.16.0-1-x86_64.pkg.tar.zst
│ └ [..]
└ [..]

```

# TODOS:
- [x] pacman cache
- [x] Patch
- [x] Implement the cli commented in src/cli.rs
- [ ] Find a solution to find pkgbase from pkgname
- [ ] dependencies (will allow groups)
- [ ] Test some big packages (base, base-devel, chromium, firefox)
- [ ] handle split pkg: List of pkgbase and a list of pkgname with a ref to pkgbase
- [ ] Get max ram usage (podman-stats)
- [ ] Keep statistics (sled)
- [ ] PKGBUILD flags `groups=('pacage')` # need doc
