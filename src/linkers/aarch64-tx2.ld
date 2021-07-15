ENTRY(_start)
SECTIONS
{
    . = 0x83000000;

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


    _bss_begin = .;
    .bss : {
        *(.bss*)
    }
    _bss_end = .;

    _image_end = ABSOLUTE(.);
    
    . = 0x400000000;

    .cpu_private (NOLOAD) : ALIGN(4096) {
        *(.cpu_private)
    }
}
