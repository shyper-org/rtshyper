use core::intrinsics::sinf64;

use super::{round_down, round_up};
use core_io as io;
use io::prelude::*;
use io::{Read, SeekFrom};
use spin::Mutex;
use fatfs::Dir;

struct Disk {
    pointer: usize,
    size: usize,
}

impl Disk {
    const fn default() -> Disk {
        Disk {
            pointer: 0,
            size: 0,
        }
    }
}

// TODO: add fatfs read
impl core_io::Read for Disk {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        // println!("in read, buf len {}", buf.len());
        let sector = round_down(self.pointer, 512) / 512;
        let offset = self.pointer - round_down(self.pointer, 512);
        let count = round_up(offset + buf.len(), 512) / 512;
        assert!(count <= 8);
        let result = crate::kernel::mem_page_alloc();
        if let Ok(frame) = result {
            // println!(
            //     "read sector {} count {} offset {} buf.len {} pointer {}",
            //     sector,
            //     count,
            //     offset,
            //     buf.len(),
            //     self.pointer
            // );
            crate::driver::read(sector, count, frame.pa());
            for i in 0..buf.len() {
                buf[i] = frame.as_slice()[offset + i];
                // print!("{}", buf[i]);
            }
            self.pointer += buf.len();
            Ok(buf.len())
        } else {
            println!("read failed");
            Ok(0)
        }
    }
}

impl core_io::Write for Disk {
    fn flush(&mut self) -> io::Result<()> {
        println!("in flush");
        Ok(())
    }

    // TODO: add fatfs write
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        println!("in write");
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

// lazy_static! {
    // static FS: Mutex<Option<fatfs::FileSystem<&mut Disk>>> = Mutex::new(None);
// }
// static ROOT_DIR: Mutex<Option<Dir<Disk>>> = Mutex::new(None);

pub fn fs_init() {
    let mut disk = Disk {
        pointer: 0,
        size: 536870912,
    };

    // // let fs = FS.lock();
    // let mut fs: io::Result<fatfs::FileSystem<&mut Disk>> =
    //     fatfs::FileSystem::new(&mut disk, fatfs::FsOptions::new());

    let mut fs = fatfs::FileSystem::new(&mut disk, fatfs::FsOptions::new()).unwrap();
    let root_dir = fs.root_dir();
    let mut file = root_dir.open_file("hello.txt");
    match file {
        Ok(mut file) => {
            let mut buf = [0u8; 13];
            // let len = file.seek(SeekFrom::End(0)).unwrap();
            file.read(&mut buf);
            for i in 0..buf.len() {
                let val = buf[i as usize];
                let tmp = char::from_u32(val as u32);
                print!("{}", tmp.unwrap());
            }
            println!("FAT file system init ok");
        }
        Err(_) => {println!("err");}
    }
}

pub fn fs_read_to_mem(filename: &str, buf: &mut [u8]) -> bool {
    let mut disk = Disk {
        pointer: 0,
        size: 536870912,
    };
    let mut fs = fatfs::FileSystem::new(&mut disk, fatfs::FsOptions::new()).unwrap();
    let root_dir = fs.root_dir();
    let mut file = root_dir.open_file(filename);
    match file {
        Ok(mut file) => {
            file.read(buf);
            return true;
        },
        Err(_) => {
            println!("read file {} failed!", filename);
            return false;
        }
    }
}

pub fn fs_file_size(filename: &str) -> usize {
    let mut disk = Disk {
        pointer: 0,
        size: 536870912,
    };
    let mut fs = fatfs::FileSystem::new(&mut disk, fatfs::FsOptions::new()).unwrap();
    let root_dir = fs.root_dir();
    let mut file = root_dir.open_file(filename);
    match file {
        Ok(mut file) => {
            return file.seek(SeekFrom::End(0)).unwrap() as usize;
        },
        Err(_) => {
            return 0;
        }
    }
}