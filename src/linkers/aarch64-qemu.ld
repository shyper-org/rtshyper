ENTRY(_start)
SECTIONS
{
    . = 0x40080000;

	_image_start = ABSOLUTE(.);

    .boot : {
        *(.text.boot)
        *(.data.boot)
    }

    .text : {
        *(.text*)
    }

    .rodata : {
        *(.rodata*)
    }


    .data : {
        *(.data*)
    }

    _image_end = ABSOLUTE(.);

    _bss_begin = .;
    .bss : {
        *(.bss*)
    }
    _bss_end = .;

    . = 0x400000000;

    .cpu_private (NOLOAD) : ALIGN(4096) {
        *(.cpu_private)
    }
}
