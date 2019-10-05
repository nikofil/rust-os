ENTRY(_start)

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
