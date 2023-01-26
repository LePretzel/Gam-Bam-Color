use crate::{mbc::MBC, memory::Memory};

const ROM_BANK_SIZE: usize = 0x4000;
const RAM_BANK_SIZE: usize = 0x2000;
pub struct MBC1 {
    rom: Vec<[u8; ROM_BANK_SIZE]>,
    ram: Vec<[u8; RAM_BANK_SIZE]>,
    ram_enabled: bool,
    rom_bank_index: u8,
    ram_bank_index: u8,
    using_ram_banking: bool,
}

impl MBC1 {
    pub fn new(rom_banks: u8, ram_banks: u8) -> Self {
        let mut mbc = MBC1 {
            rom: Vec::with_capacity(rom_banks as usize),
            ram: Vec::with_capacity(ram_banks as usize),
            ram_enabled: false,
            rom_bank_index: 0,
            ram_bank_index: 0,
            using_ram_banking: false,
        };
        // Initialize rom and ram_banks
        for _ in 0..rom_banks {
            mbc.rom.push([0; ROM_BANK_SIZE]);
        }
        for _ in 0..ram_banks {
            mbc.ram.push([0; RAM_BANK_SIZE]);
        }
        mbc
    }
}

impl Memory for MBC1 {
    fn read(&self, address: u16) -> u8 {
        match address {
            rom_bank_one_address @ 0x0000..=0x3FFF => {
                let extra_bits = 0b01100000 & self.rom_bank_index; // Will always be zero unless banks 0x20, 0x40, 0x60 are possible to access
                self.rom[0 | (extra_bits << 5) as usize][rom_bank_one_address as usize]
            }
            other_rom_banks_address @ 0x4000..=0x7FFF => {
                let bank_number = if self.rom_bank_index == 0 {
                    1
                } else {
                    self.rom_bank_index
                };
                self.rom[bank_number as usize][(other_rom_banks_address - 0x4000) as usize]
            }
            external_ram_address @ 0xA000..=0xBFFF if self.ram_enabled => {
                self.ram[self.ram_bank_index as usize][(external_ram_address - 0xA000) as usize]
            }
            _ => 0xFF,
        }
    }

    fn write(&mut self, address: u16, data: u8) {
        match address {
            // Ram enable register
            0x0000..=0x1FFF => {
                let data = data & 0b00001111;
                self.ram_enabled = if data == 0x0A { true } else { false };
            }
            // Rom bank select register
            0x2000..=0x3FFF => {
                let mask = if data as usize > self.rom.len() {
                    // Cut off bits if the rom bank would be too high for the cartridge
                    self.rom.len() as u8 - 1
                } else {
                    // Else use the bottom five bits
                    0b00011111
                };
                let data = mask & data;

                let extra_bits = if self.using_ram_banking && self.rom.len() > 32 {
                    self.ram_bank_index
                } else {
                    0
                };

                self.rom_bank_index = data | (extra_bits << 5);
            }
            // Ram bank select register
            0x4000..=0x5FFF => {
                if self.using_ram_banking {
                    if self.ram.len() > 1 || self.rom.len() >= 64 {
                        self.ram_bank_index = data;
                    }
                }
            }
            // Banking mode select register
            0x6000..=0x7FFF => {
                if data == 1 {
                    self.using_ram_banking = true;
                } else if data == 0 {
                    self.using_ram_banking = false;
                }
            }
            external_ram_address @ 0xA000..=0xBFFF
                if self.ram_enabled && self.using_ram_banking =>
            {
                self.ram[self.ram_bank_index as usize][(external_ram_address - 0xA000) as usize] =
                    data;
            }
            _ => (),
        }
    }
}

impl MBC for MBC1 {
    fn init_write(&mut self, address: u16, data: u8) {
        match address {
            rom_bank_one_address @ 0x0000..=0x3FFF => {
                let extra_bits = 0b01100000 & self.rom_bank_index;
                self.rom[0 | (extra_bits << 5) as usize][rom_bank_one_address as usize] = data;
            }
            other_rom_banks_address @ 0x4000..=0x7FFF => {
                let bank_number = if self.rom_bank_index == 0 {
                    1
                } else {
                    self.rom_bank_index
                };
                self.rom[bank_number as usize][(other_rom_banks_address - 0x4000) as usize] = data;
            }
            external_ram_address @ 0xA000..=0xBFFF if self.ram_enabled => {
                self.ram[self.ram_bank_index as usize][(external_ram_address - 0xA000) as usize] =
                    data;
            }
            _ => (),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn get_test_mbc() -> MBC1 {
        MBC1::new(0x7F + 2, 4)
    }

    #[test]
    fn ram_gets_enabled_when_correct_value_is_written() {
        let mut mbc = get_test_mbc();
        let initial = mbc.ram_enabled;
        mbc.write(0x0000, 0x0A);
        assert_ne!(initial, mbc.ram_enabled);
    }

    #[test]
    fn ram_enable_check_ignores_top_nibble_of_data() {
        let mut mbc = get_test_mbc();
        let initial = mbc.ram_enabled;
        mbc.write(0x0000, 0xFA);
        assert_ne!(initial, mbc.ram_enabled);
    }

    #[test]
    fn ram_does_not_get_enabled_with_incorrect_value() {
        let mut mbc = get_test_mbc();
        let initial = mbc.ram_enabled;
        mbc.write(0x0000, 0xFE);
        assert_eq!(initial, mbc.ram_enabled);
    }

    #[test]
    fn ram_gets_disabled_when_incorrect_value_is_written_after_enabling() {
        let mut mbc = get_test_mbc();
        let initial = mbc.ram_enabled;
        mbc.write(0x0000, 0x0A);
        assert_ne!(initial, mbc.ram_enabled);
        mbc.write(0x1000, 0xFF);
        assert_eq!(initial, mbc.ram_enabled);
    }

    #[test]
    fn can_access_rom_bank_zero() {
        let mut mbc = get_test_mbc();
        mbc.init_write(0x0000, 0x11);
        let data = mbc.read(0x0000);
        assert_eq!(data, 0x11);
    }

    #[test]
    fn can_access_rom_bank_one() {
        let mut mbc = get_test_mbc();
        mbc.write(0x2000, 0);
        mbc.init_write(0x4000, 0x11);
        let data = mbc.read(0x4000);
        assert_eq!(data, 0x11);
    }

    #[test]
    fn writes_to_rom_bank_2_do_not_affect_rom_bank_one() {
        let mut mbc = get_test_mbc();
        mbc.write(0x2000, 2);
        mbc.init_write(0x4000, 0x11);
        mbc.write(0x2000, 1);
        let data = mbc.read(0x4000);
        assert_eq!(data, 0x00);
    }

    #[test]
    fn can_access_ram_bank_zero() {
        let mut mbc = get_test_mbc();
        mbc.write(0x0000, 0x0A); // enable ram
        let data = mbc.read(0xA000);
        assert_eq!(data, 0)
    }
}
