# Neotron OS

This is the Neotron OS. It will run on any system which can execute ARM Thumb v7-M instructions, and has an implementation of the [Neotron BIOS](https://github.com/neotron-compute/Neotron-BIOS).

## Status

This OS is a work in progress. We intend to support:

* [x] Calling BIOS APIs
* [ ] Text mode console (on both text and bitmap displays)
* [ ] Starting a command-line shell application
* [ ] Executing applications from RAM
* [ ] MBR/FAT32 formatted block devices with standard open/close/read/write file semantics
* [ ] Basic networking
* [ ] Music playback
* [ ] Various keyboard layouts

## Build Instructions

```
$ git clone https://github.com/neotron-compute/Neotron-OS.git
$ cd Neotron-OS
$ git submodule update --init
$ nano ./Cargo.toml # Edit to use appropriate linker script
$ cargo build --release
$ cargo run --release # Fires up GDB to flash the board
```

## Changelog

### Unreleased Changes ([Source](https://github.com/neotron-compute/Neotron-OS/tree/master))

* Basic UART hello on start-up

## Licence

    Neotron-OS Copyright (c) The Neotron Developers, 2020

    This program is free software: you can redistribute it and/or modify
    it under the terms of the GNU General Public License as published by
    the Free Software Foundation, either version 3 of the License, or
    (at your option) any later version.

    This program is distributed in the hope that it will be useful,
    but WITHOUT ANY WARRANTY; without even the implied warranty of
    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
    GNU General Public License for more details.

    You should have received a copy of the GNU General Public License
    along with this program.  If not, see <https://www.gnu.org/licenses/>.

## Contribution

Unless you explicitly state otherwise, any contribution intentionally
submitted for inclusion in the work by you shall be licensed as above, without
any additional terms or conditions.


