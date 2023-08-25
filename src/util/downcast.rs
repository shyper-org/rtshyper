use alloc::sync::Arc;
use core::any::Any;

pub trait Downcast {
    fn as_any(&self) -> &dyn Any;
}

impl<T: Any> Downcast for T {
    fn as_any(&self) -> &dyn Any {
        self
    }
}

pub trait DowncastSync: Downcast + Send + Sync {
    fn into_any_arc(self: Arc<Self>) -> Arc<dyn Any + Send + Sync>;
}

impl<T: Any + Send + Sync> DowncastSync for T {
    fn into_any_arc(self: Arc<Self>) -> Arc<dyn Any + Send + Sync> {
        self
    }
}
