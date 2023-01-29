use crate::memory::Memory;

pub mod mbc1;
pub mod mbc3;
pub mod mbc5;

pub trait MBC: Memory {
    fn init(&mut self, program: &Vec<u8>);
}
