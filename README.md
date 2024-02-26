# Portarch
Selectively compile which arch package to locally compile for more customisation. Kindof like geento style (portage+arch).

## Usage:
Add a new entry to the `/etc/pacman.conf` with the output path of *pacage*, should be before `[core]` and `[extra]`:
```
[...]

[pacage]
SigLevel = Optional TrustAll
Server = file://<SERVER_DIR>

[...]
```

## Server dir
```bash
├ pkgs/                # Packages sources dirs
│ ├ some_package/
│ └ [..]
├ pacage_build.sh
├ pacage.db@ -> pacage.db.tar.gz
├ pacage.db.tar.gz
├ pacage.files@ -> pacage.files.tar.gz
├ pacage.files.tar.gz
├ some_package-0.16.0-1-x86_64.pkg.tar.zst
└ [..]

```

# TODOS:
- [x] Parse conf
- [x] Downloads listed pkgs
- [x] Compile downloaded pkgs
- [x] Command::output() merge stdout/stderr
- [x] Handle errors
- [ ] Daemon mod
- [ ] Logger
- [x] Build flags
- [ ] PKGBUILD flags `groups=('pacage')`
- [ ] Keep build files
- [ ] Container run / Container exec ala cross-rs
