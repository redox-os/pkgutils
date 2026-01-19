# Redox OS pkgutils

This repository contains utilities for package management on Redox. Currently, only `pkg` is included.

[![Travis Build Status](https://travis-ci.org/redox-os/pkgutils.svg?branch=master)](https://travis-ci.org/redox-os/pkgutils)
[![MIT licensed](https://img.shields.io/badge/license-MIT-blue.svg)](./LICENSE)

## `pkg` commands

The command `pkg` is the primary package management utility for Redox OS. In its current state, `pkg` supports the following commands:

```
pkg install [-y] [--reinstall] [packages]...
    Install one or more packages in glob-based pattern and its dependencies. 
    Only packages that are not installed will be installed.
    Use --reinstall if you wish to reinstall existing packages.

pkg update [-y] [--check] [packages]...
    Update one of more packages when update available.
    If package lists omitted, it will update all installed packages.
    Use --check to not actually update packages.

pkg uninstall [-y] [--all] [--force] [packages]...
    Uninstall one of more packages.
    Use --all to uninstall all packages except protected packages:
       kernel, base, base-initfs, redoxfs, ion, pkgutils.
    Use --force to also uninstall all packages that depends them.

pkg search [packages]...
    Search available packages in glob-based pattern

pkg info [package]
    Show metadata of specified package

pkg list
    Show list of installed packages

pkg postinstall
    Run all pending postinstall scripts

pkg path [path]
    Translate a path to user environment, used for postinstall scripts
```

If stdin is not a pty or `CI` is set, `-y` will be implied. If `TERM` is unset or [`NO_COLOR=1`](https://no-color.org/), the progress bar will be in plain mode.

## Development: Testing

To run tests, run

```sh
cargo test -p redox-pkg --features indicatif 
```

## Development: How it works

Redox OS `pkg` utils have a lot of files to work with. This documents internal workings for pkgutils:

### Servers `repo.toml`

A `repo.toml` file is a server file that saves nothing than a package list and their blake3 hash. The hash here is used to see if update available from our latest development. Here's [an example](https://static.redox-os.org/pkg/x86_64-unknown-redox/repo.toml):

```toml
[packages]
acid = "57fab5d366d0c9eff5f92477b10776fdc37784c3639527b844e6916dd3fba0f4"
dev-essential = ""
gcc13 = "73df1164974dc7dd4afab29d33257830c664167f3dc6adf39c9ecb67561e69b8"
"gcc13.cxx" = "0329d75f75473c278a0992d4ba28ef4439e72ebb130a8e5fcf561be4bbcf31da"
```

The empty string in `dev-essential` here means it's a meta-package, contains a group of packages. The dependencies is in each package TOML file.

Some large packages like `gcc13` here are splitted into core package an optional package `gcc13.cxx` (differentiated with a dot character). To install all optional packages in a package, use `pkg install "gcc13.*"`.

### Package TOML file

Each package has a TOML file contains it's content can the `pkg` library can read before actually downloads a pkgar (Redox OS package archive format) file. Here's how it looks:

```toml
name = "gcc13"
version = "13.2.0"
target = "x86_64-unknown-redox"
blake3 = "73df1164974dc7dd4afab29d33257830c664167f3dc6adf39c9ecb67561e69b8"
source_identifier = "no_tar_blake3_hash_info"
commit_identifier = "34e306cc762987d478c7dd7aa9c820992680ca7f"
time_identifier = "2025-12-29T18:52:27Z"
storage_size = 625837659
network_size = 625837659
depends = ["gnu-binutils", "libgmp", "libmpfr", "mpc", "zlib"]
```

When pkgutils install or update packages, it will read `repo.toml` then recurse all package TOML files for packages that will be downloaded.

### Local `/etc/pkg.d/`

This directory contains list of files that contains remote URLs of packages. This has defaults to `https://static.redox-os.org/pkg` but it can also has additional third-party URLs as well.

### `id_ed25519.pub.toml` 

This file contains public keys to all packages in a repository, this file must exist in both remote (before install) and locally (after install). Remote cannot have this file in multiple locations for a single subdomain. Locally, this file is saved to:

- `/tmp/pkg/pub_key_*.toml`: A locally cached file of remote public key per remote subdomain. This is cached so the file will only downloaded once until reboot.
- `/pkg/id_ed25519.pub.toml`: A public key from installer, this file is needed to update or remove package files built from the installer. Additionally allows local package install for development.

In unlikey event of remote has changed it public keys, the user will be notified and will be forced to remove all packages with old keys. Also it will be a hard fail if `-y` used.

### Local `/pkg/packages/*.pkgar_head`

This file contains the header part of packages already installed. This file is kept to allow update or removal of existing package files, and debugging already installed packages.

### Local `/etc/pkg/packages.toml`

This file contains the important state of already installed packages. Contents:

```toml
# protected packages name
protected = [ "kernel", ... ]
# list of installed public keys
[public_keys]
"static.redox-os.org" = { pkey = "..." }
"localhost" = { pkey = "..." }
# list of installed packages and their tree
[installed_packages.kernel]
remote = "localhost"
blake3 = "..."
manual = true # tells if user explicitly install this
dependencies = [...]
dependents = [...]
```

### Local `/var/pkg/postinstall/`

This directory contains list of scripts that will be run in `pkg postinstall` script. The script will be run in `sh` environment and will be deleted after successful operation.

### Local `/etc`

Some packages shipped with preconfigured configuration in `/etc`. However, users can specify and override configuration before and after installation. So, pkg will append `.pkgconf` to any file in `/etc` if the configuration file exists and the content differ. This is also the same when uninstalling.

### Non-Sudo Installation

A standard installation install everything in root environment. However pkgutils can also be run in non-root environment, for example it can be installed into `/home/user`. Here's how it will be mapped:

- `/usr` -> `/home/user/.local`
- `/bin` -> `/home/user/.local/bin`
- `/include` -> `/home/user/.local/include`
- `/lib` -> `/home/user/.local/lib`
- `/libexec` -> `/home/user/.local/libexec`
- `/sbin` -> `/home/user/.local/sbin`
- `/share` -> `/home/user/.local/share`
- `/etc` -> `/home/user/.config`
- `/var` -> `/home/user/.var`
- `/tmp` -> `/home/user/.cache`
- `/root` -> `/home/user`
- Any other paths -> `/home/user/.local/state`

These translation paths can be examined from `pkg path [path]` and all internal package paths mentioned above will reflect into these paths. In pkg CLI, these paths changed are automatically applied when `geteuid()` is not `0` and `$HOME` is set and not `/root`.
