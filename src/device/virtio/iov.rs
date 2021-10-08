use crate::lib::memcpy;
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::slice::from_raw_parts;
use spin::Mutex;

#[derive(Clone)]
pub struct VirtioIov {
    inner: Arc<Mutex<VirtioIovInner>>,
}

impl VirtioIov {
    pub fn default() -> VirtioIov {
        VirtioIov {
            inner: Arc::new(Mutex::new(VirtioIovInner::default())),
        }
    }

    pub fn push_data(&self, buf: usize, len: usize) {
        let mut inner = self.inner.lock();
        inner.vector.push(VirtioIovData { buf, len });
    }

    pub fn get_buf(&self, idx: usize) -> usize {
        let inner = self.inner.lock();
        inner.vector[idx].buf
    }

    pub fn num(&self) -> usize {
        let inner = self.inner.lock();
        inner.vector.len()
    }

    pub fn get_len(&self, idx: usize) -> usize {
        let inner = self.inner.lock();
        inner.vector[idx].len
    }

    pub fn get_ptr(&self, size: usize) -> &[u8] {
        let inner = self.inner.lock();
        let mut iov_idx = 0;
        let mut idx = size as isize;
        while idx >= 0 {
            if inner.vector[iov_idx].len > idx as usize {
                return unsafe {
                    from_raw_parts((inner.vector[iov_idx].buf + idx as usize) as *const u8, 14)
                };
            } else {
                iov_idx += 1;
                idx -= inner.vector[iov_idx].len as isize;
            }
            if iov_idx == inner.vector.len() {
                break;
            }
        }
        println!("iov get_ptr failed");
        return &[0];
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

            let mut written = 0;
            if dst_vlen_remain > src_vlen_remain {
                written = src_vlen_remain;
                unsafe {
                    memcpy(dst_ptr as *const u8, src_ptr as *const u8, written);
                }
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
                unsafe {
                    memcpy(dst_ptr as *const u8, src_ptr as *const u8, written);
                }
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
            remain -= written;
        }

        remain
    }
}

struct VirtioIovData {
    buf: usize,
    len: usize,
}

struct VirtioIovInner {
    vector: Vec<VirtioIovData>,
}

impl VirtioIovInner {
    pub fn default() -> VirtioIovInner {
        VirtioIovInner { vector: Vec::new() }
    }
}
