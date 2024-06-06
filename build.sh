#!/bin/bash

set -euo pipefail

SCRIPT_DIR=$( cd -- "$( dirname -- "${BASH_SOURCE[0]}" )" &> /dev/null && pwd )

RELEASE_DIR=${SCRIPT_DIR}/release

mkdir -p ${RELEASE_DIR}

# Build the embedded binaries for each core type and each flash layout
for TARGET_ARCH in thumbv6m-none-eabi thumbv7m-none-eabi thumbv7em-none-eabi; do
  echo "TARGET is ${TARGET_ARCH}"
  # Rename our utilities to have an ELF extension
  for utility in flames; do
    ( cd ${SCRIPT_DIR} && cargo build $* --release --target=${TARGET_ARCH} --bin ${utility} )
    rust-strip ${SCRIPT_DIR}/target/${TARGET_ARCH}/release/${utility} -o ${SCRIPT_DIR}/target/${TARGET_ARCH}/release/${utility}.elf
  done
  # Make a ROMFS
  export ROMFS_PATH=${SCRIPT_DIR}/target/${TARGET_ARCH}/release/romfs.img
  neotron-romfs-mkfs \
    ${SCRIPT_DIR}/target/${TARGET_ARCH}/release/flames.elf \
    > ${ROMFS_PATH}
  neotron-romfs-lsfs ${ROMFS_PATH}
  # Build the OS again, with the new ROMFS
  for BINARY in flash0002 flash0802 flash1002; do
    echo "BINARY is ${BINARY}"
    ( cd ${SCRIPT_DIR}/neotron-os && cargo build $* --release --target=${TARGET_ARCH} --bin ${BINARY} )
    # objcopy would do the build for us first, but it doesn't have good build output
    rust-objcopy -O binary ${SCRIPT_DIR}/target/${TARGET_ARCH}/release/${BINARY} ${RELEASE_DIR}/${TARGET_ARCH}-${BINARY}-libneotron_os.bin
    # Keep the ELF file too (for debugging)
    cp ${SCRIPT_DIR}/target/${TARGET_ARCH}/release/${BINARY} ${RELEASE_DIR}/${TARGET_ARCH}-${BINARY}-libneotron_os.elf
  done
done

# Build the host version
echo "Building HOST"
( cd ${SCRIPT_DIR} && cargo build --verbose --lib --release --target=x86_64-unknown-linux-gnu )
cp ${SCRIPT_DIR}/target/x86_64-unknown-linux-gnu/release/libneotron_os.so ${RELEASE_DIR}/x86_64-unknown-linux-gnu-libneotron_os.so
