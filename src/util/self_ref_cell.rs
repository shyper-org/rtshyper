use core::ptr::NonNull;

pub struct SelfRefCell<T: ?Sized> {
    value: NonNull<T>,
}

impl<T: ?Sized> Clone for SelfRefCell<T> {
    fn clone(&self) -> Self {
        Self { value: self.value }
    }
}

impl<T> SelfRefCell<T> {
    pub fn new(value: &T) -> Self {
        Self {
            value: unsafe { NonNull::new_unchecked(value as *const _ as *mut _) },
        }
    }

    pub fn as_ref<'a>(&self) -> &'a T {
        // SAFETY: the caller must guarantee that `self` meets all the
        // requirements for a reference.
        unsafe { self.value.as_ref() }
    }
}

impl<T: ?Sized> core::ops::Deref for SelfRefCell<T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &Self::Target {
        unsafe { self.value.as_ref() }
    }
}
