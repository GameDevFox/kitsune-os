ENTRY(_start)

SECTIONS
{
    . = 0x8000; /* Start here */

    /* For AArch64, use . = 0x80000; */
    __start = .;

    __boot_start = .;
    .text.boot : {
        *(.text.boot)
    }
    __boot_end = .;
    __boot_size = __boot_end - __boot_start;
    . = ALIGN(4096); /* align to page size */

    __image_data_start = .;
    .rodata.image : {
        image/* (.data)
    }
    __image_data_end = .;
    . = ALIGN(4096); /* align to page size */

    __exception_vector_start = .;
    .text.exception_vector : {
        *(.text.exception_vector)
    }
    __exception_vector_end = .;
    . = ALIGN(4096); /* align to page size */

    __exception_stacks_start = .;
        __exception_stack_data_abort = .;
        . += 0x1000;
        __exception_stack_undefined_instruction = .;
        . += 0x1000;
        __exception_stack_irq = .;
        . += 0x1000;
        __exception_stack_fiq = .;
        . += 0x1000;
    __exception_stacks_end = .;

    __data_start = .;
    .data : {
        *(.data)
    }
    __data_end = .;
    . = ALIGN(4096); /* align to page size */

    __rodata_start = .;
    .rodata : {
        *(.rodata)
    }
    __rodata_end = .;
    . = ALIGN(4096); /* align to page size */

    __bss_start = .;
    .bss : {
        *(.bss)
    }
    . = ALIGN(4096); /* align to page size */
    __bss_end = .;
    __bss_size = __bss_end - __bss_start;

    __text_start = .;
    .text : {
        *(.text)
    }
    __text_end = .;
    __text_size = __text_end - __text_start;
    . = ALIGN(4096); /* align to page size */

    __end = .;
}
