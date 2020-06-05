# Redox OS pkgutils

This repository contains utilities for package management on Redox. Currently, only `pkg` is included.

[![Travis Build Status](https://travis-ci.org/redox-os/pkgutils.svg?branch=master)](https://travis-ci.org/redox-os/pkgutils)
[![MIT licensed](https://img.shields.io/badge/license-MIT-blue.svg)](./LICENSE)

## `pkg`
The command `pkg` is the primary package management utility for Redox OS. In its current state, `pkg` supports the following commands:

| Command   | Functionality                  |
|-----------|--------------------------------|
| `clean`   | Clean an extracted package     |
| `create`  | Create a package               |
| `extract` | Extract a package              |
| `fetch`   | Download a package             |
| `install` | Install a package              |
| `list`    | List package contents          |
| `sign`    | Get a file signature           |
| `upgrade` | Upgrade all installed packages |

For more detailed information on how to invoke these subcommands, please run `pkg help <SUBCOMMAND>` in your terminal.
