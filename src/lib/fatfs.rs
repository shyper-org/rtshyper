use super::{round_down, round_up};
use crate::arch::PAGE_SIZE;
use core_io as io;
use io::prelude::*;
use io::{Read, SeekFrom};

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
        let sector = round_down(self.pointer, 512) / 512;
        let offset = self.pointer - round_down(self.pointer, 512);
        let count = round_up(offset + buf.len(), 512) / 512;
        assert!(count <= 8);
        let result = crate::kernel::mem_page_alloc();
        if let Ok(frame) = result {
            // if count >= 4 {
            // println!(
            //     "read sector {} count {} offset {} buf.len {} pointer {}",
            //     sector,
            //     count,
            //     offset,
            //     buf.len(),
            //     self.pointer
            // );
            // }
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
        // println!("in flush");
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

    let fs = fatfs::FileSystem::new(&mut disk, fatfs::FsOptions::new()).unwrap();
    let root_dir = fs.root_dir();
    let file = root_dir.open_file("hello.txt");
    match file {
        Ok(mut file) => {
            let mut buf = [0u8; 5000];
            // let len = file.seek(SeekFrom::End(0)).unwrap();
            file.read(&mut buf);
            file.read(&mut buf[4096..]);
            let mut idx = 0;
            for i in 0..buf.len() {
                let val = buf[i as usize];
                if val != 0 {
                    idx += 1;
                }
                let tmp = char::from_u32(val as u32);
                // print!("{}", tmp.unwrap());
            }
            // println!("idx is {}", idx);
            println!("FAT file system init ok");
        }
        Err(_) => {
            println!("err");
        }
    }
}

pub fn fs_read_to_mem(filename: &str, buf: &mut [u8]) -> bool {
    let mut disk = Disk {
        pointer: 0,
        size: 536870912,
    };
    let count = round_up(buf.len(), PAGE_SIZE) / PAGE_SIZE;

    let fs = fatfs::FileSystem::new(&mut disk, fatfs::FsOptions::new()).unwrap();
    let root_dir = fs.root_dir();
    let file = root_dir.open_file(filename);
    match file {
        Ok(mut file) => {
            for i in 0..count {
                if i + 1 != count {
                    file.read(&mut buf[i * PAGE_SIZE..(i + 1) * PAGE_SIZE]);
                } else {
                    file.read(&mut buf[i * PAGE_SIZE..]);
                }
            }
            return true;
        }
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
    let fs = fatfs::FileSystem::new(&mut disk, fatfs::FsOptions::new()).unwrap();
    let root_dir = fs.root_dir();
    let file = root_dir.open_file(filename);
    match file {
        Ok(mut file) => {
            return file.seek(SeekFrom::End(0)).unwrap() as usize;
        }
        Err(_) => {
            return 0;
        }
    }
}
