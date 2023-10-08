#!/bin/bash

set -euo pipefail

RELEASE_DIR=./release

mkdir -p ${RELEASE_DIR}

# Build the embedded binaries for each core type and each flash layout
for TARGET_ARCH in thumbv6m-none-eabi thumbv7m-none-eabi thumbv7em-none-eabi; do
  echo "TARGET is ${TARGET_ARCH}"
  for BINARY in flash0002 flash0802 flash1002; do
    echo "BINARY is ${BINARY}"
    cargo build $* --release --target=${TARGET_ARCH} --bin ${BINARY}
    # objcopy would do the build for us first, but it doesn't have good build output
    rust-objcopy -O binary ./target/${TARGET_ARCH}/release/${BINARY} ${RELEASE_DIR}/${TARGET_ARCH}-${BINARY}-libneotron_os.bin
    # Keep the ELF file too (for debugging)
    cp ./target/${TARGET_ARCH}/release/${BINARY} ${RELEASE_DIR}/${TARGET_ARCH}-${BINARY}-libneotron_os.elf
  done
done

# Build the host version
echo "Building HOST"
cargo build --verbose --lib --release --target=x86_64-unknown-linux-gnu
cp ./target/x86_64-unknown-linux-gnu/release/libneotron_os.so ${RELEASE_DIR}/x86_64-unknown-linux-gnu-libneotron_os.so
