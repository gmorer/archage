# Portarch
Selectively compile which arch package to locally compile for more customisation. Kindof like geento style (portage+arch).

## Usage:
Add a new entry to the `/etc/pacman.conf` with the output path of archage, should be before `[core]` and `[extra]`:
```
[...]

[archage]
SigLevel = Optional TrustAll
Server = file://<SERVER_DIR>

[...]
```

# TODOS:
- [x] Parse conf
- [x] Downloads listed pkgs
- [x] Compile downloaded pkgs
- [ ] Command::output() merge stdout/stderr
- [ ] Handle errors
- [ ] Daemon mod
- [ ] Build flags
