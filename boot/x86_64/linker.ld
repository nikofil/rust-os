ENTRY(_setup_stack)

SECTIONS {
    . = 1M;

    .boot :
    {
        *(.multiboot_header)
    }

    .setup_stack :
    {
        *(.setup_stack)
    }

    .text :
    {
        *(.text)
    }
}
