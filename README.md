# Neotron OS

This is the Neotron OS. It will run on any system which can execute ARM Thumb v7-M instructions, and has an implementation of the [Neotron BIOS](https://github.com/neotron-compute/Neotron-BIOS).

## Status

This OS is a work in progress. We intend to support:

* [x] Calling BIOS APIs
* [x] Text mode VGA console
* [x] Serial console
* [ ] Starting a command-line shell application
* [ ] Executing applications from RAM
* [ ] MBR/FAT32 formatted block devices with standard open/close/read/write file semantics
* [ ] Basic networking
* [ ] Music playback
* [ ] Various keyboard layouts
* [ ] Ethernet / WiFi networking

## Build instructions

Your board will need an appropriate Neotron BIOS installed, and you need to
have OpenOCD (or other programming tool) running for your particular board.
You may also need to set the linker arguments so you link the binary to suit
the memory available on your system.

### Build Instructions for the Neotron Pico (and other systems with Flash at `0x1000_0000`)

```
$ git clone https://github.com/neotron-compute/Neotron-OS.git
$ cd Neotron-OS
$ git submodule update --init
$ RUSTFLAGS="-C link-arg=-Tneotron-flash-1000.ld" cargo build --release --target=thumbv6m-none-eabi
```

### Build Instructions for the STM32 (and other systems with Flash at `0x0800_0000`)

```
$ git clone https://github.com/neotron-compute/Neotron-OS.git
$ cd Neotron-OS
$ git submodule update --init
$ RUSTFLAGS="-C link-arg=-Tneotron-flash-0800.ld" cargo build --release --target=thumbv6m-none-eabi
```

### Build Instructions for other systems (with Flash at `0x0000_0000`)

```
$ git clone https://github.com/neotron-compute/Neotron-OS.git
$ cd Neotron-OS
$ git submodule update --init
$ RUSTFLAGS="-C link-arg=-Tneotron-flash-0000.ld" cargo run --release
```

## Changelog

### Unreleased Changes ([Source](https://github.com/neotron-compute/Neotron-OS/tree/master))

* Basic `println!` to the text buffer.

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


