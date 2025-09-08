use crate::mbc::MBC;
use crate::memory::Memory;

use crate::registers::{
    BCPD_ADDRESS, BCPS_ADDRESS, DIV_ADDRESS, OCPD_ADDRESS, OCPS_ADDRESS, SVBK_ADDRESS, VBK_ADDRESS,
};

pub struct MemManager {
    memory: [u8; 0xFFFF + 1],
    vram_bank_one: [u8; 0x2000 + 1],
    extra_ram_banks: [[u8; 0x1000 + 1]; 6],
    object_palettes: [u8; 64],
    background_palettes: [u8; 64],
    mbc: Option<Box<dyn MBC>>,
}

impl MemManager {
    pub fn new() -> Self {
        MemManager {
            memory: [0; 0xFFFF + 1],
            vram_bank_one: [0; 0x2000 + 1],
            extra_ram_banks: [[0; 0x1000 + 1]; 6],
            object_palettes: [0; 64],
            background_palettes: [0; 64],
            mbc: None,
        }
    }

    pub fn force_write(&mut self, address: u16, data: u8) {
        self.memory[address as usize] = data;
    }

    pub fn print_memory(&self, start: u16, end: u16) {
        print!("{:x}: ", start);
        for (i, address) in (start..=end).enumerate() {
            if i > 0 && i % 16 == 0 {
                println!();
                print!("{:x}: ", address);
            }
            print!("{:x} ", self.read(address));
        }
        println!()
    }

    pub fn print_palettes(&self) {
        print!("Background palettes:");
        for i in (0..self.background_palettes.len()).step_by(2) {
            if i % 8 == 0 {
                println!();
            }
            print!(
                "{:x}{:x} ",
                self.background_palettes[i + 1],
                self.background_palettes[i],
            );
        }
        println!();
        print!("Object palettes:");
        for i in (0..self.object_palettes.len()).step_by(2) {
            if i % 8 == 0 {
                println!();
            }
            print!(
                "{:x}{:x} ",
                self.object_palettes[i + 1],
                self.object_palettes[i],
            );
        }
        println!();
    }

    pub fn set_mbc(&mut self, mbc: Option<Box<dyn MBC>>) {
        self.mbc = mbc;
    }
}

impl Memory for MemManager {
    fn read(&self, address: u16) -> u8 {
        let ram_bank = self.memory[SVBK_ADDRESS as usize] & 0b00000111;
        let vram_bank = self.memory[VBK_ADDRESS as usize] & 0b00000001;
        match address {
            rom_address @ 0x0000..=0x7FFF if self.mbc.is_some() => {
                self.mbc.as_ref().unwrap().read(rom_address)
            }
            external_ram_address @ 0xA000..=0xBFFF if self.mbc.is_some() => {
                self.mbc.as_ref().unwrap().read(external_ram_address)
            }
            ram_banks_address @ 0xD000..=0xDFFF if ram_bank > 1 => {
                self.extra_ram_banks[(ram_bank - 2) as usize][(ram_banks_address - 0xD000) as usize]
            }
            vram_address @ 0x8000..=0x9FFF if vram_bank == 1 => {
                self.vram_bank_one[(vram_address - 0x8000) as usize]
            }
            OCPD_ADDRESS => {
                let palette_index = self.memory[OCPS_ADDRESS as usize] & 0b00111111;
                self.object_palettes[palette_index as usize]
            }
            BCPD_ADDRESS => {
                let palette_index = self.memory[BCPS_ADDRESS as usize] & 0b00111111;
                self.background_palettes[palette_index as usize]
            }
            _ => {
                let result = self.memory[address as usize];
                result
            }
        }
    }

    fn write(&mut self, address: u16, data: u8) {
        let ram_bank = self.memory[SVBK_ADDRESS as usize] & 0b00000111;
        let vram_bank = self.memory[VBK_ADDRESS as usize] & 0b00000001;

        match address {
            joyp_address @ 0xFF00 => {
                // Ignore writes to the lower 4 bits of this register since they are only
                // used to detect if the game is running on super gameboy hardware
                let curr_value = self.memory[joyp_address as usize] & 0b00001111;
                self.memory[address as usize] = (data & 0b11110000) | curr_value;
            }
            rom_address @ 0x0000..=0x7FFF if self.mbc.is_some() => {
                self.mbc.as_mut().unwrap().write(rom_address, data);
            }
            external_ram_address @ 0xA000..=0xBFFF if self.mbc.is_some() => {
                self.mbc.as_mut().unwrap().write(external_ram_address, data);
            }
            ram_banks_address @ 0xD000..=0xDFFF if ram_bank > 1 => {
                self.extra_ram_banks[(ram_bank - 2) as usize]
                    [(ram_banks_address - 0xD000) as usize] = data
            }
            vram_address @ 0x8000..=0x9FFF if vram_bank == 1 => {
                self.vram_bank_one[(vram_address - 0x8000) as usize] = data
            }
            OCPD_ADDRESS => {
                let ocps = self.memory[OCPS_ADDRESS as usize];
                let palette_index = ocps & 0b00111111;
                self.object_palettes[palette_index as usize] = data;
                let auto_increment = ocps & 0b10000000 != 0;
                if auto_increment {
                    self.memory[OCPS_ADDRESS as usize] =
                        (ocps & 0b10000000) | palette_index.wrapping_add(1);
                }
            }
            BCPD_ADDRESS => {
                let bcps = self.memory[BCPS_ADDRESS as usize];
                let palette_index = bcps & 0b00111111;
                self.background_palettes[palette_index as usize] = data;
                let auto_increment = bcps & 0b10000000 != 0;
                if auto_increment {
                    self.memory[BCPS_ADDRESS as usize] =
                        (bcps & 0b10000000) | palette_index.wrapping_add(1);
                }
            }
            DIV_ADDRESS => self.memory[address as usize] = 0,
            _ => self.memory[address as usize] = data,
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::registers::DIV_ADDRESS;

    use super::*;

    #[test]
    fn writing_to_div_sets_it_to_zero() {
        let mut mem = MemManager::new();
        mem.write(DIV_ADDRESS, 0x45);
        assert_eq!(mem.read(DIV_ADDRESS), 0x00);
    }

    #[test]
    fn ram_bank_two_is_accesible() {
        let mut mem = MemManager::new();
        mem.write(SVBK_ADDRESS, 0x02);
        mem.write(0xD000, 0xAA);
        assert_eq!(mem.read(0xD000), 0xAA);
    }

    #[test]
    fn ram_bank_7_is_accesible() {
        let mut mem = MemManager::new();
        mem.write(SVBK_ADDRESS, 0x07);
        mem.write(0xDFFF, 0xAA);
        assert_eq!(mem.read(0xDFFF), 0xAA);
    }

    #[test]
    fn ram_bank_two_does_not_change_bank_zero() {
        let mut mem = MemManager::new();
        mem.write(VBK_ADDRESS, 0x02);
        mem.write(0xDEAD, 0xAA);
        mem.write(VBK_ADDRESS, 0x00);
        assert_eq!(mem.read(0xDFFF), 0x00);
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

    #[test]
    fn ocps_selects_bcpd() {
        let mut mem = MemManager::new();
        mem.write(OCPS_ADDRESS, 0b00000011);
        mem.write(OCPD_ADDRESS, 0xAA);
        mem.write(OCPS_ADDRESS, 0b00000001);
        mem.write(OCPD_ADDRESS, 0xBB);
        assert_eq!(mem.read(OCPD_ADDRESS), 0xBB);
        mem.write(OCPS_ADDRESS, 0b00000011);
        assert_eq!(mem.read(OCPD_ADDRESS), 0xAA);
    }

    #[test]
    fn ocps_auto_increments() {
        let mut mem = MemManager::new();
        mem.write(OCPS_ADDRESS, 0b10000000);
        mem.write(OCPD_ADDRESS, 0xAA);
        mem.write(OCPD_ADDRESS, 0xBB);
        mem.write(OCPD_ADDRESS, 0xCC);
        mem.write(OCPS_ADDRESS, 0b00000001);
        assert_eq!(mem.read(OCPD_ADDRESS), 0xBB);
    }

    #[test]
    fn bcps_selects_bcpd() {
        let mut mem = MemManager::new();
        mem.write(BCPS_ADDRESS, 0b00000011);
        mem.write(BCPD_ADDRESS, 0xAA);
        mem.write(BCPS_ADDRESS, 0b00000001);
        mem.write(BCPD_ADDRESS, 0xBB);
        assert_eq!(mem.read(BCPD_ADDRESS), 0xBB);
        mem.write(BCPS_ADDRESS, 0b00000011);
        assert_eq!(mem.read(BCPD_ADDRESS), 0xAA);
    }

    #[test]
    fn bcps_auto_increments() {
        let mut mem = MemManager::new();
        mem.write(BCPS_ADDRESS, 0b10000000);
        mem.write(BCPD_ADDRESS, 0xAA);
        mem.write(BCPD_ADDRESS, 0xBB);
        mem.write(BCPD_ADDRESS, 0xCC);
        mem.write(BCPS_ADDRESS, 0b00000001);
        assert_eq!(mem.read(BCPD_ADDRESS), 0xBB);
    }
}
