/**
 * Neotron OS Application linker script.
 *
 * The Neotron OS has no RAM allocated - all memory allocation is provided by
 * the BIOS through callback functions. Write this image to anywhere in flash
 * (or load into RAM) and tell the BIOS about it. The BIOS will adjust the
 * entry pointer appropriately, and the rest of the code is position
 * independent, so it will just work - in theory!.
 *
 * Copyright (c) Jonathan 'theJPster' Pallant 2019
 * Copyright (c) Rust Embedded Working Group 2018
 *
 * Available under the MIT or Apache 2.0 licence, at your option.
 */


MEMORY
{
    FLASH (rx)  : ORIGIN = 0x00000000, LENGTH = 0x00040000
    /* No RAM required */
}

EXTERN(ENTRY_POINT);

SECTIONS
{
    .entry ORIGIN(FLASH) :
    {
        KEEP(*(.entry_point))
    } > FLASH

    .text :
    {
        *    (.text .text.*)
        *    (.init)
        *    (.fini)
    } > FLASH

    .rodata : ALIGN(4)
    {
        *(.rodata .rodata.*);
        /* 4-byte align the end (VMA) of this section.
           This is required by LLD to ensure the LMA of the following .data
           section will have the correct alignment. */
        . = ALIGN(4);
    } > FLASH

    /* ## Discarded sections */
    /DISCARD/ :
    {
        /* Unused exception related info that only wastes space */
        *(.ARM.exidx*);
    }
}
