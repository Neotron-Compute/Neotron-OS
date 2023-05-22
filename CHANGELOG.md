# Change Log

## Unreleased changes

## v0.3.3

* Add `dir` command
* Change `load` command to load from disk
* Repository includes `Cargo.lock` file
* Update to `postcard` 1.0
* Fix `readblk` help text, and print 32 bytes per line

## v0.3.2

* Add `date` command.
* Add `lsblk` and `blkread` commands.
* Renamed `bioshw` to `lshw`

## v0.3.1

* Add `hexdump`, `load` and `run` commands.
* Set colour attributes correctly (White on Black only currently)

## v0.3.0

* Updated to Neotron Common BIOS v0.8.0
* Use pc-keyboard for decoding HID events
* Fix Windows library build
* Added 'kbtest' command
* Added 'lshw' command
* Added 'config' command
* Uses BIOS to store/load OS configuration

## v0.2.0

Adds HID support and basic shell, with 'mem' and 'fill' commands.

## v0.1.0

First version.
