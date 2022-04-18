# Neotron OS

This is the Neotron OS. It will run on any system which has an implementation
of the [Neotron BIOS](https://github.com/neotron-compute/Neotron-BIOS).

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

Your board will need an appropriate Neotron BIOS installed, and you need to have
OpenOCD or some other programming tool running for your particular board. See
your BIOS instructions for more details.

We compile one version of Neotron OS, but we link it three times to produce
three different binaries:

* `flash0002` - is linked to run from address `0x0002_0000`
* `flash1002` - is linked to run from address `0x1002_0000`
* `flash0802` - is linked to run from address `0x0802_0000`

```console
$ git clone https://github.com/neotron-compute/Neotron-OS.git
$ cd Neotron-OS
$ cargo build --target thumbv6m-none-eabi --release --bins
$ ls ./target/thumbv6m-none-eabi/release/flash*02
./target/thumbv6m-none-eabi/release/flash0002 ./target/thumbv6m-none-eabi/release/flash0802 ./target/thumbv6m-none-eabi/release/flash1002
```

Your BIOS should tell you which one you want and how to load it onto your system.

You can also build a *shared object* to load into a Windows/Linux/macOS application.

```console
$ cargo build --lib
$ ls ./target/debug/*.so
./target/debug/libneotron_os.so
```

## Changelog

### Unreleased Changes ([Source](https://github.com/neotron-compute/Neotron-OS/tree/master))

* Basic `println!` to the text buffer.
* Re-arranged linker script setup

## Licence

    Neotron-OS Copyright (c) The Neotron Developers, 2022

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


