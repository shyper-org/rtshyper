cmake_minimum_required(VERSION 3.9)
project(libfdt-binding)

set(CMAKE_BUILD_TYPE "Release")

set(CMAKE_C_STANDARD 11)
set(CMAKE_C_COMPILER aarch64-none-elf-gcc)
set(CMAKE_ASM_COMPILER aarch64-none-elf-gcc)

include_directories(.)
include_directories(../deps/libfdt)

aux_source_directory(../deps/libfdt LIBFDT_SRC)

add_library(fdt-binding STATIC
  ${LIBFDT_SRC}
  "wrapper.c"
  "wrapper.h"
  "mystr.c"
)
