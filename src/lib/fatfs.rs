use core::intrinsics::sinf64;

use super::{round_down, round_up};
use core_io as io;
use io::prelude::*;
use io::{Read, SeekFrom};
use spin::Mutex;

struct Disk {
    pointer: usize,
    size: usize,
}

impl Disk {
    const fn default() -> Disk {
        Disk {
            pointer: 0,
            size: 0
        }
    }
}

// TODO: add fatfs read
impl core_io::Read for Disk {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        Ok(buf.len())
    }
}

impl core_io::Write for Disk {
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }

    // TODO: add fatfs write
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        println!("write dropped");
        Ok(0)
    }
}

impl core_io::Seek for Disk {
    fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        match pos {
            SeekFrom::Start(u) => {
                self.pointer = u as usize;
            }
            SeekFrom::End(i) => {
                self.pointer = self.size - (i as usize);
            }
            SeekFrom::Current(i) => {
                self.pointer += i as usize;
            }
        }
        Ok(self.pointer as u64)
    }
}

// static FS: Option<Mutex<io::Result<fatfs::FileSystem<Disk>>>> = None;

pub fn fs_init() {
    let mut disk = Disk {
        pointer: 0,
        size: 536870912,
    };

    // // let fs = FS.lock();
    // let mut fs: io::Result<fatfs::FileSystem<&mut Disk>> =
    //     fatfs::FileSystem::new(&mut disk, fatfs::FsOptions::new());

    let mut fs = fatfs::FileSystem::new(&mut disk, fatfs::FsOptions::new());
    // TODO: check this function, maybe cannot work
    println!("FAT file system init ok");
}
