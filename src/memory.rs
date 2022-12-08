pub trait Memory {
    fn read(&self, address: u16) -> u8;
    fn write(&mut self, address: u16, data: u8);
    fn read_u16(&self, address: u16) -> u16;
    fn write_u16(&mut self, address: u16, data: u16);
}

pub struct MemManager {
    memory: [u8; 0xFFFF + 1],
    vram_bank_one: [u8; 0x2000 + 1],
    object_palettes: [u8; 64],
    background_palettes: [u8; 64],
}

impl MemManager {
    pub fn new() -> Self {
        MemManager {
            memory: [0; 0xFFFF + 1],
            vram_bank_one: [0; 0x2000 + 1],
            object_palettes: [0; 64],
            background_palettes: [0; 64],
        }
    }

    pub fn force_write(&mut self, address: u16, data: u8) {
        self.memory[address as usize] = data;
    }
}

const DIV_ADDRESS: u16 = 0xFF04;
const BCPS_ADDRESS: u16 = 0xFF68;
const BCPD_ADDRESS: u16 = 0xFF69;
const OCPS_ADDRESS: u16 = 0xFF6A;
const OCPD_ADDRESS: u16 = 0xFF6B;
const VBK_ADDRESS: u16 = 0xFF4F;

impl Memory for MemManager {
    fn read(&self, address: u16) -> u8 {
        let vram_bank = self.memory[VBK_ADDRESS as usize] & 0b00000001;
        match address {
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
            _ => self.memory[address as usize],
        }
    }

    fn write(&mut self, address: u16, data: u8) {
        let vram_bank = self.memory[VBK_ADDRESS as usize] & 0b00000001;
        match address {
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
                        (ocps & 0b11000000) | palette_index.wrapping_add(1);
                }
            }
            BCPD_ADDRESS => {
                let bcps = self.memory[BCPS_ADDRESS as usize];
                let palette_index = bcps & 0b00111111;
                self.background_palettes[palette_index as usize] = data;
                let auto_increment = bcps & 0b10000000 != 0;
                if auto_increment {
                    self.memory[BCPS_ADDRESS as usize] =
                        (bcps & 0b11000000) | palette_index.wrapping_add(1);
                }
            }
            DIV_ADDRESS => self.memory[address as usize] = 0,
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
