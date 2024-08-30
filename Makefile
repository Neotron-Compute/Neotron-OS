.PHONY: clean all rebuild

OUTPUT_DIR = ./release

TARGET_LIST = \
	thumbv6m-none-eabi \
	thumbv7m-none-eabi \
	thumbv7em-none-eabi

OFFSET_LIST = \
	flash0002 \
	flash0802 \
	flash1002 \

ROMFS_BIN_LIST = \
	flames

CROSS_OS_FILES_BIN = $(foreach target,$(TARGET_LIST),$(foreach offset,$(OFFSET_LIST), $(OUTPUT_DIR)/$(target)-$(offset)-libneotron_os.bin))

CROSS_OF_FILES_ELF = $(CROSS_OS_FILES_BIN:.bin=.elf)

LINUX_SO = $(OUTPUT_DIR)/x86_64-unknown-linux-gnu-libneotron_os.so

all: $(LINUX_SO) $(CROSS_OF_FILES_ELF) $(CROSS_OS_FILES_BIN) $(ROM_IMAGE)

clean:
	rm -rf $(OUTPUT_DIR)
	cargo clean

rebuild: clean all

#
# Building the disk images
#

$(OUTPUT_DIR)/thumbv6m-none-eabi-romfs.img: $(OUTPUT_DIR)/thumbv6m-none-eabi-romfs/flames.elf
	neotron-romfs-mkfs $^ > $@

$(OUTPUT_DIR)/thumbv6m-none-eabi-romfs/flames.elf: ./target/thumbv6m-none-eabi/release/flames
	mkdir -p $(OUTPUT_DIR)/thumbv6m-none-eabi-romfs
	rust-strip $^ -o $@

./target/thumbv6m-none-eabi/release/flames:
	cargo build --release --bin=flames --target=thumbv6m-none-eabi

-include ./target/thumbv6m-none-eabi/release/flames.d

$(OUTPUT_DIR)/thumbv7m-none-eabi-romfs.img: $(OUTPUT_DIR)/thumbv7m-none-eabi-romfs/flames.elf
	neotron-romfs-mkfs $^ > $@

$(OUTPUT_DIR)/thumbv7m-none-eabi-romfs/flames.elf: ./target/thumbv7m-none-eabi/release/flames
	mkdir -p $(OUTPUT_DIR)/thumbv7m-none-eabi-romfs
	rust-strip $^ -o $@

./target/thumbv7m-none-eabi/release/flames:
	cargo build --release --bin=flames --target=thumbv7m-none-eabi

-include ./target/thumbv7m-none-eabi/release/flames.d

$(OUTPUT_DIR)/thumbv7em-none-eabi-romfs.img: $(OUTPUT_DIR)/thumbv7em-none-eabi-romfs/flames.elf
	neotron-romfs-mkfs $^ > $@

$(OUTPUT_DIR)/thumbv7em-none-eabi-romfs/flames.elf: ./target/thumbv7em-none-eabi/release/flames
	mkdir -p $(OUTPUT_DIR)/thumbv7em-none-eabi-romfs
	rust-strip $^ -o $@

./target/thumbv7em-none-eabi/release/flames:
	cargo build --release --bin=flames --target=thumbv7em-none-eabi

-include ./target/thumbv7em-none-eabi/release/flames.d

#
# Building the Linux shared-object
#

$(LINUX_SO): ./target/x86_64-unknown-linux-gnu/release/libneotron_os.so
	mkdir -p $(OUTPUT_DIR)
	cp $^ $@

./target/x86_64-unknown-linux-gnu/release/libneotron_os.so: 
	cargo build --lib --release --target=x86_64-unknown-linux-gnu

-include ./target/x86_64-unknown-linux-gnu/release/libneotron_os.d

#
# Converting Rust .elf files into .bin files
#

%.bin: %.elf
	rust-objcopy -O binary $^ $@

$(OUTPUT_DIR)/thumbv6m-none-eabi-flash0002-libneotron_os.elf: ./target/thumbv6m-none-eabi/release/flash0002
	cp $^ $@

$(OUTPUT_DIR)/thumbv6m-none-eabi-flash0802-libneotron_os.elf: ./target/thumbv6m-none-eabi/release/flash0802
	cp $^ $@

$(OUTPUT_DIR)/thumbv6m-none-eabi-flash1002-libneotron_os.elf: ./target/thumbv6m-none-eabi/release/flash1002
	cp $^ $@

$(OUTPUT_DIR)/thumbv7em-none-eabi-flash0002-libneotron_os.elf: ./target/thumbv7em-none-eabi/release/flash0002
	cp $^ $@

$(OUTPUT_DIR)/thumbv7em-none-eabi-flash0802-libneotron_os.elf: ./target/thumbv7em-none-eabi/release/flash0802
	cp $^ $@

$(OUTPUT_DIR)/thumbv7em-none-eabi-flash1002-libneotron_os.elf: ./target/thumbv7em-none-eabi/release/flash1002
	cp $^ $@

$(OUTPUT_DIR)/thumbv7m-none-eabi-flash0002-libneotron_os.elf: ./target/thumbv7m-none-eabi/release/flash0002
	cp $^ $@

$(OUTPUT_DIR)/thumbv7m-none-eabi-flash0802-libneotron_os.elf: ./target/thumbv7m-none-eabi/release/flash0802
	cp $^ $@

$(OUTPUT_DIR)/thumbv7m-none-eabi-flash1002-libneotron_os.elf: ./target/thumbv7m-none-eabi/release/flash1002
	cp $^ $@


#
# flash0002 binaries
#

./target/thumbv6m-none-eabi/release/flash0002: $(OUTPUT_DIR)/thumbv6m-none-eabi-romfs.img
	ROMFS=$(OUTPUT_DIR)/thumbv6m-none-eabi-romfs.img cargo build --release --target=thumbv6m-none-eabi --bin=flash0002

-include ./target/thumbv6m-none-eabi/release/flash0002.d

./target/thumbv7m-none-eabi/release/flash0002: $(OUTPUT_DIR)/thumbv7m-none-eabi-romfs.img
	ROMFS=$(OUTPUT_DIR)/thumbv7m-none-eabi-romfs.img cargo build --release --target=thumbv7m-none-eabi --bin=flash0002

-include ./target/thumbv7m-none-eabi/release/flash0002.d

./target/thumbv7em-none-eabi/release/flash0002: $(OUTPUT_DIR)/thumbv7em-none-eabi-romfs.img
	ROMFS=$(OUTPUT_DIR)/thumbv7em-none-eabi-romfs.img cargo build --release --target=thumbv7em-none-eabi --bin=flash0002

-include ./target/thumbv7em-none-eabi/release/flash0002.d

#
# flash0802 binaries
#

./target/thumbv6m-none-eabi/release/flash0802: $(OUTPUT_DIR)/thumbv6m-none-eabi-romfs.img
	ROMFS=$(OUTPUT_DIR)/thumbv6m-none-eabi-romfs.img cargo build --release --target=thumbv6m-none-eabi --bin=flash0802

-include ./target/thumbv6m-none-eabi/release/flash0802.d

./target/thumbv7m-none-eabi/release/flash0802: $(OUTPUT_DIR)/thumbv7m-none-eabi-romfs.img
	ROMFS=$(OUTPUT_DIR)/thumbv7m-none-eabi-romfs.img cargo build --release --target=thumbv7m-none-eabi --bin=flash0802

-include ./target/thumbv7m-none-eabi/release/flash0802.d

./target/thumbv7em-none-eabi/release/flash0802: $(OUTPUT_DIR)/thumbv7em-none-eabi-romfs.img
	ROMFS=$(OUTPUT_DIR)/thumbv7em-none-eabi-romfs.img cargo build --release --target=thumbv7em-none-eabi --bin=flash0802

-include ./target/thumbv7em-none-eabi/release/flash0802.d

#
# flash1002 binaries
#

./target/thumbv6m-none-eabi/release/flash1002: $(OUTPUT_DIR)/thumbv6m-none-eabi-romfs.img
	ROMFS=$(OUTPUT_DIR)/thumbv6m-none-eabi-romfs.img cargo build --release --target=thumbv6m-none-eabi --bin=flash1002

-include ./target/thumbv6m-none-eabi/release/flash1002.d

./target/thumbv7m-none-eabi/release/flash1002: $(OUTPUT_DIR)/thumbv7m-none-eabi-romfs.img
	ROMFS=$(OUTPUT_DIR)/thumbv7m-none-eabi-romfs.img cargo build --release --target=thumbv7m-none-eabi --bin=flash1002

-include ./target/thumbv7m-none-eabi/release/flash1002.d

./target/thumbv7em-none-eabi/release/flash1002: $(OUTPUT_DIR)/thumbv7em-none-eabi-romfs.img
	ROMFS=$(OUTPUT_DIR)/thumbv7em-none-eabi-romfs.img cargo build --release --target=thumbv7em-none-eabi --bin=flash1002

-include ./target/thumbv7em-none-eabi/release/flash1002.d
