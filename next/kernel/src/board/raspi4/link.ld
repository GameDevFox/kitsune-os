ENTRY(_start)

SECTIONS
{
    /* Where the Pi firmware and QEMU load a 64-bit kernel. */
    . = 0x80000;

    /* The stack occupies the memory below the kernel. */
    __stack_top = .;

    /* Where boot.s saves the firmware's device tree pointer. It's outside the kernel,
       so `cargo pi` loading a new one leaves it alone, and past the other fixed areas in
       `board/raspi4/mod.rs` and `audio.rs`. */
    __device_tree_slot = 0x14000000;

    .text : {
        KEEP(*(.text.boot))
        *(.text .text.*)
    }

    .rodata : ALIGN(8) {
        *(.rodata .rodata.*)
    }

    .data : ALIGN(8) {
        *(.data .data.*)
    }

    /* Both ends are 8-byte aligned because boot.s clears a word at a time. */
    .bss (NOLOAD) : ALIGN(8) {
        __bss_start = .;
        *(.bss .bss.*)
        *(COMMON)
        . = ALIGN(8);
        __bss_end = .;
    }

    /* Images live from 0x200000, in `board/raspi4/mod.rs`, and `cargo pi` replaces
       only what's before them. */
    ASSERT(. <= 0x200000, "the kernel has grown into the image region")

    /DISCARD/ : {
        *(.comment)
        *(.eh_frame*)
    }
}
