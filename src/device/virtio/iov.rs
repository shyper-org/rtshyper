use alloc::sync::Arc;
use alloc::vec::Vec;
use core::slice::from_raw_parts;

use spin::Mutex;

use crate::util::memcpy_safe;

#[derive(Clone)]
pub(super) struct VirtioIov {
    inner: Arc<Mutex<VirtioIovInner>>,
}

impl VirtioIov {
    pub fn default() -> VirtioIov {
        VirtioIov {
            inner: Arc::new(Mutex::new(VirtioIovInner::default())),
        }
    }

    pub fn clear(&self) {
        let mut inner = self.inner.lock();
        inner.vector.clear();
    }

    pub fn push_data(&self, buf: usize, len: usize) {
        let mut inner = self.inner.lock();
        inner.vector.push(VirtioIovData { buf, len });
    }

    pub fn get_buf(&self, idx: usize) -> usize {
        let inner = self.inner.lock();
        inner.vector[idx].buf
    }

    pub fn to_buf(&self, addr: usize, len: usize) {
        let mut size = len;
        let inner = self.inner.lock();
        for iov_data in &inner.vector {
            let offset = len - size;
            let dst = addr + offset;
            if iov_data.len >= size {
                memcpy_safe(dst as *const u8, iov_data.buf as *const u8, size);
                break;
            } else {
                memcpy_safe(dst as *const u8, iov_data.buf as *const u8, iov_data.len);
                size -= iov_data.len;
            }
        }
    }

    pub fn from_buf(&self, addr: usize, len: usize) {
        let mut size = len;
        let inner = self.inner.lock();
        for iov_data in &inner.vector {
            let offset = len - size;
            let src = addr + offset;
            if iov_data.len >= size {
                memcpy_safe(iov_data.buf as *const u8, src as *const u8, size);
                break;
            } else {
                memcpy_safe(iov_data.buf as *const u8, src as *const u8, iov_data.len);
                size -= iov_data.len;
            }
        }
    }

    pub fn num(&self) -> usize {
        let inner = self.inner.lock();
        inner.vector.len()
    }

    pub fn get_len(&self, idx: usize) -> usize {
        let inner = self.inner.lock();
        inner.vector[idx].len
    }

    pub fn get_ptr(&self, size: usize) -> &'static [u8] {
        let inner = self.inner.lock();
        // let mut iov_idx = 0;
        let mut idx = size;

        for iov_data in &inner.vector {
            if iov_data.len > idx {
                if iov_data.buf + idx < 0x1000 {
                    panic!("illegal addr {:x}", iov_data.buf + idx);
                }
                return unsafe { from_raw_parts((iov_data.buf + idx) as *const u8, 14) };
            } else {
                idx -= iov_data.len;
            }
        }

        println!("iov get_ptr failed");
        println!("get_ptr iov {:#?}", inner.vector);
        println!("size {}, idx {}", size, idx);
        &[0]
    }

    pub fn write_through_iov(&self, dst: VirtioIov, remain: usize) -> usize {
        let inner = self.inner.lock();

        let mut dst_iov_idx = 0;
        let mut src_iov_idx = 0;
        let mut dst_ptr = dst.get_buf(0);
        let mut src_ptr = inner.vector[0].buf;
        let mut dst_vlen_remain = dst.get_len(0);
        let mut src_vlen_remain = inner.vector[0].len;
        let mut remain = remain;
        // println!(
        //     "dst_vlen_remain {}, src_vlen_remain {}, remain {}",
        //     dst_vlen_remain, src_vlen_remain, remain
        // );

        while remain > 0 {
            if dst_iov_idx == dst.num() || src_iov_idx == inner.vector.len() {
                break;
            }

            let written;
            if dst_vlen_remain > src_vlen_remain {
                written = src_vlen_remain;
                if dst_ptr < 0x1000 || src_ptr < 0x1000 {
                    panic!("illegal des addr {:x}, src addr {:x}", dst_ptr, src_ptr);
                }
                memcpy_safe(dst_ptr as *const u8, src_ptr as *const u8, written);
                src_iov_idx += 1;
                if src_iov_idx < inner.vector.len() {
                    src_ptr = inner.vector[src_iov_idx].buf;
                    src_vlen_remain = inner.vector[src_iov_idx].len;
                    dst_ptr += written;
                    dst_vlen_remain -= written;
                }
                // if dst_vlen_remain == 0 {
                //     dst_iov_idx += 1;
                //     dst_ptr = dst.get_buf(dst_iov_idx);
                //     dst_vlen_remain = dst.get_len(dst_iov_idx);
                // }
            } else {
                written = dst_vlen_remain;
                if dst_ptr < 0x1000 || src_ptr < 0x1000 {
                    panic!("illegal des addr {:x}, src addr {:x}", dst_ptr, src_ptr);
                }
                memcpy_safe(dst_ptr as *const u8, src_ptr as *const u8, written);
                dst_iov_idx += 1;
                if dst_iov_idx < dst.num() {
                    dst_ptr = dst.get_buf(dst_iov_idx);
                    dst_vlen_remain = dst.get_len(dst_iov_idx);
                    src_ptr += written;
                    src_vlen_remain -= written;
                }
                if inner.vector[src_iov_idx].len == 0 {
                    src_iov_idx += 1;
                    if src_iov_idx < inner.vector.len() {
                        src_ptr = inner.vector[src_iov_idx].buf;
                        src_vlen_remain = inner.vector[src_iov_idx].len;
                    }
                }
            }
            // if remain < written {
            //     println!("remain {} less than writter {}", remain, written);
            //     return 1;
            // }
            remain -= written;
        }

        remain
    }
}

#[derive(Debug)]
struct VirtioIovData {
    buf: usize,
    len: usize,
}

#[derive(Debug)]
struct VirtioIovInner {
    vector: Vec<VirtioIovData>,
}

impl VirtioIovInner {
    pub fn default() -> VirtioIovInner {
        VirtioIovInner { vector: Vec::new() }
    }
}
