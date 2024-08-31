.PHONY: clean all rebuild

#
# User variables
#
# These are things you are expected to maybe need to edit
#

OUTPUT_DIR = ./release

TARGET_LIST = \
	thumbv6m-none-eabi \
	thumbv7m-none-eabi \
	thumbv7em-none-eabi

UTILITY_LIST = \
	flames \
	show \

FLASHOFFSET_LIST = \
	flash0002 \
	flash0802 \
	flash1002 \

#
# Private variables
#
# These are things you are not expected to need to edit
#

LINUX_SO = $(OUTPUT_DIR)/x86_64-unknown-linux-gnu-libneotron_os.so

all: $(LINUX_SO)

clean:
	rm -rf $(OUTPUT_DIR)
	cargo clean

rebuild: clean all

#
# Macros
#
# These are macros we use to build rules
#

# This is all the things that must be done per-target, per-utility
define TARGET_UTILITY_MACRO

# Build the stripped utility from the unstripped utility
$(OUTPUT_DIR)/$(1)-utilities/$(2).elf: ./target/$(1)/release/$(2)
	mkdir -p $$(@D)
	rust-strip $$^ -o $$@

# Build the unstripped utility from the Rust source
./target/$(1)/release/$(2):
	cargo build --release --bin=$(2) --target=$(1)

endef

# This is all the things that must be done per-target, per-flashoffset
define TARGET_FLASHOFFSET_MACRO

# Build the raw binary from the ELF
$(OUTPUT_DIR)/$(1)-$(2)-libneotron_os.bin: $(OUTPUT_DIR)/$(1)-$(2)-libneotron_os.elf
	mkdir -p $$(@D)
	rust-objcopy -O binary $$^ $$@

all: $(OUTPUT_DIR)/$(1)-$(2)-libneotron_os.bin

# Put the ELF in the release area
$(OUTPUT_DIR)/$(1)-$(2)-libneotron_os.elf: ./target/$(1)/release/$(2)
	cp $$^ $$@

# Build the ELF from the rust source
./target/$(1)/release/$(2): $(OUTPUT_DIR)/$(1)-romfs.img
	ROMFS_PATH=$(abspath $$<) cargo build --release --bin=$(2) --target=$(1)

-include ./target/$(1)/release/$(2).d

endef

# This is all the things that must be done per-target
define TARGET_MACRO

ROMFS_STRIPPED_LIST = $(foreach bin,$(UTILITY_LIST),$(OUTPUT_DIR)/$(1)-utilities/$(bin).elf)

# Build the ROMFS image for this target from the list of stripped utilities
$(OUTPUT_DIR)/$(1)-romfs.img: $$(ROMFS_STRIPPED_LIST)
	neotron-romfs-mkfs $$^ > $$@

all: $(OUTPUT_DIR)/$(1)-romfs.img

$(foreach bin,$(UTILITY_LIST), $(eval $(call TARGET_UTILITY_MACRO,$(1),$(bin))))

$(foreach flashoffset,$(FLASHOFFSET_LIST), $(eval $(call TARGET_FLASHOFFSET_MACRO,$(1),$(flashoffset))))

endef

#
# Rules
#
# These are our rules for building things
#

$(foreach target,$(TARGET_LIST),$(eval $(call TARGET_MACRO,$(target))))

#
# Building the Linux shared-object
#

$(LINUX_SO): ./target/x86_64-unknown-linux-gnu/release/libneotron_os.so
	mkdir -p $(OUTPUT_DIR)
	cp $^ $@

./target/x86_64-unknown-linux-gnu/release/libneotron_os.so: 
	cargo build --lib --release --target=x86_64-unknown-linux-gnu

-include ./target/x86_64-unknown-linux-gnu/release/libneotron_os.d
