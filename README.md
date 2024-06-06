# Neotron OS

This is the Neotron OS. It will run on any system which has an implementation
of the [Neotron BIOS](https://github.com/neotron-compute/Neotron-Common-BIOS).

## Status

This OS is a work in progress. We intend to support:

* [x] Calling BIOS APIs
* [x] Text mode VGA console
* [x] Serial console
* [x] Running built-in commands from a shell
* [x] Executing applications from RAM
  * [x] Applications can print to stdout
  * [x] Applications can read from stdin
  * [ ] Applications can open/close/read/write files
* [x] MBR/FAT32 formatted block devices
  * [x] Read blocks
  * [x] Directory listing of /
  * [ ] Write to files
  * [ ] Delete files
  * [ ] Change directory
* [x] Load ELF binaries from disk
* [x] Changing text modes
* [ ] Basic networking
* [x] Music playback
* [ ] Various keyboard layouts
* [ ] Ethernet / WiFi networking
* [ ] Built-in scripting language

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

If you want to include a ROMFS, you need to:

```bash
cargo install neotron-romfs-lsfs
cargo install neotron-romfs-mkfs
cargo install cargo-binutils
```

A bunch of utilities are supplied in the [`utilities`](./utilities/) folder. Build them all, and make a ROMFS image, then build the OS with the `ROMFS_PATH` environment variable set.

```bash
TGT=$(pwd)/target/thumbv6m-none-eabi/release
cargo build --bin flames --target thumbv6m-none-eabi --release
rust-strip ${TGT}/flames -o ${TGT}/flames.elf
neotron-romfs-mkfs ${TGT}/flames.elf > ${TGT}/romfs.img
ROMFS_PATH=${TGT}/romfs.img cargo build --bin flash1002 --target thumbv6m-none-eabi --release
```

The OS will then include the ROMFS image, which you can access with the `rom` command.

```text
> rom
flames.elf (14212 bytes)
> rom flames.elf
Loading 4256 bytes to 0x20001000
Loading 532 bytes to 0x200020a0
Loading 4908 bytes to 0x200022b4
> run
*Program starts running**
```

A better UI for loading files from ROM is being planned (maybe we should have drive letters, and the ROM can be `R:`).

## Changelog

See [`CHANGELOG.md`](./CHANGELOG.md)

## Licence

```text
Neotron-OS Copyright (c) Jonathan 'theJPster' Pallant and The Neotron Developers, 2023

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
```

See the full text in [LICENSE.txt](./LICENSE.txt). Broadly, we (the developers)
interpret this to mean (and note that we are not lawyers and this is not
legal advice) that if you give someone a Neotron computer, you must also give them
one of:

* Complete and corresponding source code (e.g. on disk, or as a link to your
  **own** on-line Git repo) for any GPL components (e.g. the BIOS and the OS),
  as supplied on the Neotron computer.
* A written offer to provide complete and corresponding source code on
  request.

If you are not offering a Neotron computer commercially (i.e. you are not
selling a board for commercial gain), and you are using an unmodified upstream
version of the source code, then the third option is to give them:

* A link to the tag/commit-hash on the relevant official Neotron Github
  repository - <https://github.com/Neotron-Compute/Neotron-OS>.

This is to ensure everyone always has the freedom to access the source code in
their Neotron based computer.

## Contribution

Unless you explicitly state otherwise, any contribution intentionally
submitted for inclusion in the work by you shall be licensed as above, without
any additional terms or conditions.


