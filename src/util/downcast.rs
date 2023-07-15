use core::any::Any;

pub trait Downcast {
    fn as_any(&self) -> &dyn Any;
}

impl<T: Any> Downcast for T {
    fn as_any(&self) -> &dyn Any {
        self
    }
}
