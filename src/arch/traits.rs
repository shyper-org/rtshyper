use crate::arch::ContextFrame;

pub trait ContextFrameTrait {
    fn new(pc: usize, sp: usize, arg: usize, privileged: bool) -> Self;

    fn exception_pc(&self) -> usize;
    fn set_exception_pc(&mut self, pc: usize);
    fn stack_pointer(&self) -> usize;
    fn set_stack_pointer(&mut self, sp: usize);
    fn set_argument(&mut self, arg: usize);
    fn gpr(&self, index: usize) -> usize;
}
