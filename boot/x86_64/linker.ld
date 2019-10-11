ENTRY(_start)

SECTIONS {
	. = 0xC0100000;

	.boot : AT (ADDR (.boot) - 0xC0000000)
	{
		*(.multiboot_header)
	}
	.text : AT (ADDR (.text) - 0xC0000000)
	{
		*(.text)
	}
}
