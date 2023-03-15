ENTRY(_start)

SECTIONS
{
    . = 0xf0080000;

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

    . = ALIGN(4096);
    _bss_begin = .;
    .bss (NOLOAD) : ALIGN(4096) {
        *(.bss*)
    }
    . = ALIGN(4096);
    _bss_end = .;

    _image_end = ABSOLUTE(.);

    . = 0x400000000;
    CPU_BASE = ABSOLUTE(.);
    .cpu_private (NOLOAD) : ALIGN(4096) {
        *(.cpu_private)
    }
}
