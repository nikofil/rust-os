ENTRY(_start)

SECTIONS {
	. = 0xC0100000;

	.boot : AT (ADDR (.boot) - 0xC0000000)
	{
        _bootstart = .;
		*(.multiboot_header)
        _bootend = .;
	}
	.rodata : AT (ADDR (.rodata) - 0xC0000000)
	{
        _rodatastart = .;
		*(.rodata*)
        _rodataend = .;
	}
	.text : AT (ADDR (.text) - 0xC0000000)
	{
        _textstart = .;
		*(.text)
        _textend = .;
	}
}
