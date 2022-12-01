use std::ops::Div;

pub trait Memory {
    fn read(&self, address: u16) -> u8;
    fn write(&mut self, address: u16, data: u8);
    fn read_u16(&self, address: u16) -> u16;
    fn write_u16(&mut self, address: u16, data: u16);
}

pub struct MemManager {
    memory: [u8; 0xFFFF + 1],
    vram_bank_one: [u8; 0x400],
}

impl MemManager {
    pub fn new() -> Self {
        MemManager {
            memory: [0; 0xFFFF + 1],
            vram_bank_one: [0; 0x400],
        }
    }

    pub fn force_write(&mut self, address: u16, data: u8) {
        self.memory[address as usize] = data;
    }
}

const DIV_ADDRESS: u16 = 0xFF04;
const VBK_ADDRESS: u16 = 0xFF4F;

impl Memory for MemManager {
    fn read(&self, address: u16) -> u8 {
        let vram_bank = self.memory[VBK_ADDRESS as usize] & 0b00000001;
        match address {
            a @ 0x8000..=0x9FFF if vram_bank == 1 => self.vram_bank_one[(a - 0x8000) as usize],
            _ => self.memory[address as usize],
        }
    }

    fn write(&mut self, address: u16, data: u8) {
        let vram_bank = self.memory[VBK_ADDRESS as usize] & 0b00000001;
        match address {
            a @ 0x8000..=0x9FFF if vram_bank == 1 => {
                self.vram_bank_one[(a - 0x8000) as usize] = data
            }
            x if x == DIV_ADDRESS => self.memory[address as usize] = 0,
            _ => self.memory[address as usize] = data,
        }
    }

    fn read_u16(&self, address: u16) -> u16 {
        let low = self.read(address) as u16;
        let high = self.read(address + 1) as u16;
        (high << 8) | low
    }

    fn write_u16(&mut self, address: u16, data: u16) {
        let low = data as u8;
        let high = (data >> 8) as u8;
        self.write(address, low);
        self.write(address + 1, high);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writing_to_div_sets_it_to_zero() {
        let mut mem = MemManager::new();
        mem.write(DIV_ADDRESS, 0x45);
        assert_eq!(mem.read(DIV_ADDRESS), 0x00);
    }

    #[test]
    fn vram_bank_one_is_accesible() {
        let mut mem = MemManager::new();
        mem.write(VBK_ADDRESS, 0x01);
        mem.write(0x8000, 0xAA);
        assert_eq!(mem.read(0x8000), 0xAA);
    }

    #[test]
    fn vram_bank_one_does_not_change_bank_zero() {
        let mut mem = MemManager::new();
        mem.write(VBK_ADDRESS, 0x01);
        mem.write(0x8000, 0xAA);
        mem.write(VBK_ADDRESS, 0x00);
        assert_eq!(mem.read(0x8000), 0x00);
    }
}
