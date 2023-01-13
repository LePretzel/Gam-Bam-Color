use crate::memory::Memory;

pub mod mbc1;

pub trait MBC: Memory {
    // Only used to initialize rom memory since the normal write methods just change mbc registers
    fn init_write(&mut self, address: u16, data: u8);
}
