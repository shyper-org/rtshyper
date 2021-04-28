use crate::arch::ContextFrame;

pub trait ContextFrameTrait {
    fn new(pc: usize, sp: usize, arg: usize, privileged: bool) -> Self;

    fn exception_pc(&self) -> usize;
    fn set_exception_pc(&mut self, pc: usize);
    fn stack_pointer(&self) -> usize;
    fn set_stack_pointer(&mut self, sp: usize);
    fn set_argument(&mut self, arg: usize);
    fn set_gpr(&mut self, index: usize, val: usize);
    fn gpr(&self, index: usize) -> usize;
}

pub trait ArchPageTableEntryTrait {
    fn from_pte(value: usize) -> Self;
    fn from_pa(pa: usize) -> Self;
    fn to_pte(&self) -> usize;
    fn to_pa(&self) -> usize;
    fn valid(&self) -> bool;
    fn entry(&self, index: usize) -> Self;
    fn set_entry(&self, index: usize, value: Self);
    fn make_table(frame_pa: usize) -> Self;
}
