#![cfg_attr(not(test), no_std)]

pub use raii_derive::RAII;

#[allow(drop_bounds)]
pub trait RaiiBound: Drop {}
