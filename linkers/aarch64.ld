ENTRY(_start)
SECTIONS
{
    . = TEXT_START;

	_image_start = ABSOLUTE(.);

    .boot : {
        *(.text.boot)
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

    .bss (NOLOAD) : ALIGN(4096) {
        *(.bss.stack)

        . = ALIGN(4096);
        _bss_begin = .;
        *(.bss*)
        . = ALIGN(4096);
        _bss_end = .;
    }

    _image_end = ABSOLUTE(.);
    
    . = 0x400000000;
    CPU_BASE = ABSOLUTE(.);
    .cpu_private (NOLOAD) : ALIGN(4096) {
        *(.cpu_private)
    }
}
