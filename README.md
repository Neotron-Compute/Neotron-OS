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
* [ ] Ethernet / WiFi networking

## Build instructions

Your board will need an appropriate Neotron BIOS installed, and you need to have OpenOCD running for your particular board. You also need to set the linker 
arguments so you link the binary to suit the memory available on your system.

### Build Instructions for the Neotron 340ST

```
$ git clone https://github.com/neotron-compute/Neotron-OS.git
$ cd Neotron-OS
$ git submodule update --init
$ RUSTFLAGS="-C link-arg=-Tneotron-os-256k.ld" cargo run --release
```

### Build Instructions for the Neotron 32

```
$ git clone https://github.com/neotron-compute/Neotron-OS.git
$ cd Neotron-OS
$ git submodule update --init
$ RUSTFLAGS="-C link-arg=-Tneotron-os-26k.ld" cargo run --release
```

TODO: Think of a better way of setting the memory limits for a particular OS build.

## Changelog

### Unreleased Changes ([Source](https://github.com/neotron-compute/Neotron-OS/tree/master))

* Basic UART hello on start-up on the Neotron 32 and Neotron 340ST

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


