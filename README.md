# Redox OS pkgutils

This repository contains utilities for package management on Redox. Currently, only `pkg` is included.

[![Travis Build Status](https://travis-ci.org/redox-os/pkgutils.svg?branch=master)](https://travis-ci.org/redox-os/pkgutils)
[![MIT licensed](https://img.shields.io/badge/license-MIT-blue.svg)](./LICENSE)

## `pkg`
The command `pkg` is the primary package management utility for Redox OS. In its current state, `pkg` supports the following commands:

| Command     | Functionality                  |
|-------------|--------------------------------|
| `install`   | Install packages               |
| `uninstall` | Uninstall packages             |
| `update`    | Update packages (all if empty) |
| `search`    | Search for a package           |
| `info`      | Package info                   |
| `list`      | List of installed packages     |

For more detailed information on how to invoke these subcommands, please run `pkg help <SUBCOMMAND>` in your terminal.
