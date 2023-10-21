# Change Log

## Unreleased changes ([Source](https://github.com/neotron-compute/neotron-os/tree/develop) | [Changes](https://github.com/neotron-compute/neotron-os/compare/v0.7.1...develop))

* None

## v0.7.1 - 2023-10-21 ([Source](https://github.com/neotron-compute/neotron-os/tree/v0.7.1) | [Release](https://github.com/neotron-compute/neotron-os/releases/tag/v0.7.1))

* Update `Cargo.lock` so build string no longer shows build as *dirty*

## v0.7.0 - 2023-10-21 ([Source](https://github.com/neotron-compute/neotron-os/tree/v0.7.0) | [Release](https://github.com/neotron-compute/neotron-os/releases/tag/v0.7.0))

* Add `i2c` command.
* Support printing `\t`, with 8 character tab-stops
* Add `type` command to print files
* Add `exec` command to execute scripts containing commands
* Update `embedded-sdmmc` crate
* Split `lshw` into `lsblk`, `lsbus`, `lsi2c`, `lsmem` and `lsuart`

## v0.6.0 - 2023-10-08 ([Source](https://github.com/neotron-compute/neotron-os/tree/v0.6.0) | [Release](https://github.com/neotron-compute/neotron-os/releases/tag/v0.6.0))

* Can set/set video mode
* Stores video mode as part of config
* Removed demo commands (they should be applications)
* Added raw PCM sound playback
* Added mixer command
* Switch to [`neotron-common-bios`] 0.11.1

## v0.5.0 - 2023-07-21 ([Source](https://github.com/neotron-compute/neotron-os/tree/v0.5.0) | [Release](https://github.com/neotron-compute/neotron-os/releases/tag/v0.5.0))

* Switch to [`neotron-common-bios`] 0.11
* Added "Shutdown" command
* Added ANSI decoder for colour changes (SGI) and cursor position support
* Added 'standard input' support for applications
* Use new compare-and-swap BIOS API to implement mutexes, instead of `static mut`
* OS now requires 256K Flash space

## v0.4.0 - 2023-06-25 ([Source](https://github.com/neotron-compute/neotron-os/tree/v0.4.0) | [Release](https://github.com/neotron-compute/neotron-os/releases/tag/v0.4.0))

* The `load` command now takes ELF binaries, not raw binaries.
* Neotron OS can now be used as a dependency within an application, if desired.

## v0.3.3 - 2023-05-22 ([Source](https://github.com/neotron-compute/neotron-os/tree/v0.3.3) | [Release](https://github.com/neotron-compute/neotron-os/releases/tag/v0.3.3))

* Add `dir` command
* Change `load` command to load from disk
* Repository includes `Cargo.lock` file
* Update to `postcard` 1.0
* Fix `readblk` help text, and print 32 bytes per line

## v0.3.2 - 2023-05-05 ([Source](https://github.com/neotron-compute/neotron-os/tree/v0.3.2) | [Release](https://github.com/neotron-compute/neotron-os/releases/tag/v0.3.2))

* Add `date` command.
* Add `lsblk` and `blkread` commands.
* Renamed `bioshw` to `lshw`

## v0.3.1 - 2023-03-09 ([Source](https://github.com/neotron-compute/neotron-os/tree/v0.3.1) | [Release](https://github.com/neotron-compute/neotron-os/releases/tag/v0.3.1))

* Add `hexdump`, `load` and `run` commands.
* Set colour attributes correctly (White on Black only currently)

## v0.3.0 - 2023-02-12 ([Source](https://github.com/neotron-compute/neotron-os/tree/v0.3.0) | [Release](https://github.com/neotron-compute/neotron-os/releases/tag/v0.3.0))

* Updated to [`neotron-common-bios`] v0.8.0
* Use [`pc-keyboard`] for decoding HID events
* Fix Windows library build
* Added 'kbtest' command
* Added 'lshw' command
* Added 'config' command
* Uses BIOS to store/load OS configuration

[`neotron-common-bios`]: https://crates.io/crates/neotron-common-bios
[`pc-keyboard`]: https://crates.io/crates/pc-keyboard

## v0.2.0 - 2023-01-07 ([Source](https://github.com/neotron-compute/neotron-os/tree/v0.2.0) | [Release](https://github.com/neotron-compute/neotron-os/releases/tag/v0.2.0))

Adds HID support and basic shell, with 'mem' and 'fill' commands.

## v0.1.0 - 2022-03-18 ([Source](https://github.com/neotron-compute/neotron-os/tree/v0.1.0) | [Release](https://github.com/neotron-compute/neotron-os/releases/tag/v0.1.0))

First version.
