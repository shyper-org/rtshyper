cmake_minimum_required(VERSION 3.9)
project(libfdt-binding)

# set(CMAKE_BUILD_TYPE "Release")

set(CMAKE_C_STANDARD 11)
set(CMAKE_C_COMPILER aarch64-elf-gcc)
set(CMAKE_ASM_COMPILER aarch64-elf-gcc)
# set(CMAKE_C_FLAGS_RELEASE "-O3")

include_directories(.)

add_library(fdt-binding STATIC
  "fdt.c"
  "fdt.h"
  "fdt_addresses.c"
  "fdt_check.c"
  "fdt_empty_tree.c"
  "fdt_overlay.c"
  "fdt_ro.c"
  "fdt_rw.c"
  "fdt_strerror.c"
  "fdt_sw.c"
  "fdt_wip.c"
  "libfdt.h"
  "libfdt_env.h"
  "libfdt_internal.h"
  "wrapper.c"
  "wrapper.h"
  "mystr.c"
)
