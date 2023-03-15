use core::mem::size_of;
use core::slice;

use super::Vm;
use crate::arch::PAGE_SIZE;
use crate::kernel::vm_ipa2hva;
use crate::util::{round_down, memcpy_safe};

pub fn copy_segment_to_vm<T: Sized>(vm: &Vm, load_ipa: usize, bin: &[T]) {
    let bin = unsafe { slice::from_raw_parts(bin.as_ptr() as *const u8, bin.len() * size_of::<T>()) };
    let offset = load_ipa - round_down(load_ipa, PAGE_SIZE);
    let start = if offset != 0 {
        info!(
            "ipa {:#x} not align to PAGE_SIZE {:#x}, length {:#x}",
            load_ipa,
            PAGE_SIZE,
            bin.len()
        );
        let hva = vm_ipa2hva(vm, load_ipa) as *mut u8;
        let size = usize::min(bin.len(), PAGE_SIZE - offset);
        memcpy_safe(hva as *mut _, bin[0..].as_ptr() as *const _, size);
        // let dst = unsafe { slice::from_raw_parts_mut(pa, size) };
        // dst.copy_from_slice(&bin[0..size]);
        size
    } else {
        0
    };
    for i in (start..bin.len()).step_by(PAGE_SIZE) {
        let hva = vm_ipa2hva(vm, load_ipa + i) as *mut u8;
        let size = usize::min(bin.len() - i, PAGE_SIZE);
        memcpy_safe(hva as *mut _, bin[i..].as_ptr() as *const _, size);
        // let dst = unsafe { slice::from_raw_parts_mut(pa, size) };
        // dst.copy_from_slice(&bin[i..i + size]);
    }
}

pub fn copy_segment_from_vm<T: Sized>(vm: &Vm, bin: &mut [T], load_ipa: usize) {
    let bin = unsafe { slice::from_raw_parts_mut(bin.as_mut_ptr() as *mut u8, bin.len() * size_of::<T>()) };
    let offset = load_ipa - round_down(load_ipa, PAGE_SIZE);
    let start = if offset != 0 {
        info!(
            "ipa {:#x} not align to PAGE_SIZE {:#x}, length {:#x}",
            load_ipa,
            PAGE_SIZE,
            bin.len()
        );
        let hva = vm_ipa2hva(vm, load_ipa) as *mut u8;
        let size = usize::min(bin.len(), PAGE_SIZE - offset);
        memcpy_safe(bin[0..].as_ptr() as *mut _, hva as *const _, size);
        // let src = unsafe { slice::from_raw_parts(pa, size) };
        // bin[0..size].clone_from_slice(src);
        size
    } else {
        0
    };
    for i in (start..bin.len()).step_by(PAGE_SIZE) {
        let hva = vm_ipa2hva(vm, load_ipa + i) as *mut u8;
        let size = usize::min(bin.len() - i, PAGE_SIZE);
        memcpy_safe(bin[i..].as_ptr() as *mut _, hva as *const _, size);
        // let src = unsafe { slice::from_raw_parts(pa, size) };
        // bin[i..i + size].clone_from_slice(src);
    }
}

pub fn copy_to_vm<T: Sized>(vm: &Vm, to: *mut u8, from: &T) {
    copy_segment_to_vm(vm, to as usize, slice::from_ref(from));
}

pub fn copy_from_vm<T: Sized>(vm: &Vm, to: &mut T, from: *const u8) {
    copy_segment_from_vm(vm, slice::from_mut(to), from as usize);
}
