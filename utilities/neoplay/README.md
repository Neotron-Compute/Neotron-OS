# Neoplay

A ProTracker MOD player for the Neotron Pico.

Runs at 11,025 Hz, quadrupling samples for the audio codec which runs at 44,100 Hz.

```console
$ cargo build --release --target=thumbv6m-none-eabi
$ cp ../target/thumbv6m-none-eabi/release/neoplay /media/USER/SDCARD/NEOPLAY.ELF

```

```console
> load neoplay.elf
> run airwolf.mod
Loading "airwolf.mod"
audio 44100, SixteenBitStereo
Playing "airwolf.mod"

000 000000 12 00fe 0f04|-- ---- ----|-- ---- ----|-- ---- ----|
000 000001 -- ---- ----|-- ---- ----|-- ---- ----|-- ---- ----|
000 000002 -- ---- ----|-- ---- ----|-- ---- ----|-- ---- ----|
000 000003 -- ---- ----|-- ---- ----|-- ---- ----|-- ---- ----|
etc
```

Here's a video of it in action: https://youtu.be/ONZhDrZsmDU
