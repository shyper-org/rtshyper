//! Wrapper type for safe pointers to device MMIO.

// inspired by https://github.com/tock/tock
// kernel/src/utilities/static_ref.rs

use core::marker::PhantomData;
use core::ops::Deref;
use core::ptr::NonNull;

/// A pointer to statically allocated mutable data such as memory mapped I/O
/// registers.
///
/// This is a simple wrapper around a raw pointer that encapsulates an unsafe
/// dereference in a safe manner. It serve the role of creating a `&'static T`
/// given a raw address and acts similarly to `extern` definitions, except
/// `DeviceRefCell` is subject to module and crate boundaries, while `extern`
/// definitions can be imported anywhere.
///
/// Because this defers the actual dereference, this can be put in a `const`,
/// whereas `const I32_REF: &'static i32 = unsafe { &*(0x1000 as *const i32) };`
/// will always fail to compile since `0x1000` doesn't have an allocation at
/// compile time, even if it's known to be a valid MMIO address.
#[derive(Debug)]
pub struct DeviceRef<'a, T> {
    ptr: NonNull<T>,
    _marker: PhantomData<&'a T>,
}

impl<T> DeviceRef<'_, T> {
    /// Create a new `DeviceRefCell` from a raw pointer
    ///
    /// ## Safety
    ///
    /// - `ptr` must be aligned, non-null, and dereferencable as `T`.
    /// - `*ptr` must be valid for the program duration.
    #[inline(always)]
    pub const unsafe fn new(ptr: *const T) -> Self {
        // SAFETY: `ptr` is non-null as promised by the caller.
        Self {
            ptr: NonNull::new_unchecked(ptr.cast_mut()),
            _marker: PhantomData,
        }
    }

    #[inline(always)]
    pub const fn dangling() -> Self {
        Self {
            ptr: NonNull::dangling(),
            _marker: PhantomData,
        }
    }

    #[inline(always)]
    pub fn addr(&self) -> usize {
        self.ptr.as_ptr() as usize
    }
}

impl<T> Copy for DeviceRef<'_, T> {}

impl<T> Clone for DeviceRef<'_, T> {
    fn clone(&self) -> Self {
        *self
    }
}

// Safety: T provides the necessary guarantees for Sync and Send
unsafe impl<T: Send> Send for DeviceRef<'_, T> {}
unsafe impl<T: Send + Sync> Sync for DeviceRef<'_, T> {}

impl<T> Deref for DeviceRef<'_, T> {
    type Target = T;

    #[inline(always)]
    fn deref(&self) -> &T {
        // SAFETY: `ptr` is aligned and dereferencable for the program
        // duration as promised by the caller of `DeviceRefCell::new`.
        unsafe { self.ptr.as_ref() }
    }
}
