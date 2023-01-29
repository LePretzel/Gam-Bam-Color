use crate::{mbc::MBC, memory::Memory};

const ROM_BANK_SIZE: usize = 0x4000;
const RAM_BANK_SIZE: usize = 0x2000;

// Todo: Implement external real time clock (RTC)
pub struct MBC3 {
    rom: Vec<[u8; ROM_BANK_SIZE]>,
    ram: Vec<[u8; RAM_BANK_SIZE]>,
    ram_enabled: bool,
    rom_bank_index: u8,
    ram_bank_index: u8,
}

impl MBC3 {
    pub fn new(rom_banks: u8, ram_banks: u8) -> Self {
        let mut mbc = MBC3 {
            rom: Vec::with_capacity(rom_banks as usize),
            ram: Vec::with_capacity(ram_banks as usize),
            ram_enabled: false,
            rom_bank_index: 0,
            ram_bank_index: 0,
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

    fn init_write(&mut self, address: u16, data: u8) {
        match address {
            rom_bank_one_address @ 0x0000..=0x3FFF => {
                self.rom[0][rom_bank_one_address as usize] = data;
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
                if (0x08..=0x0C).contains(&self.ram_bank_index) {
                    // Used for RTC registers (not implemented)
                    return;
                }
                self.ram[self.ram_bank_index as usize][(external_ram_address - 0xA000) as usize] =
                    data;
            }
            _ => (),
        }
    }
}

impl Memory for MBC3 {
    fn read(&self, address: u16) -> u8 {
        match address {
            rom_bank_one_address @ 0x0000..=0x3FFF => self.rom[0][rom_bank_one_address as usize],
            other_rom_banks_address @ 0x4000..=0x7FFF => {
                let bank_number = if self.rom_bank_index == 0 {
                    1
                } else {
                    self.rom_bank_index
                };
                self.rom[bank_number as usize][(other_rom_banks_address - 0x4000) as usize]
            }
            external_ram_address @ 0xA000..=0xBFFF if self.ram_enabled => {
                if (0x08..=0x0C).contains(&self.ram_bank_index) {
                    // Used for RTC registers (not implemented)
                    return 0xFF;
                }
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
                self.rom_bank_index = data;
            }
            // Ram bank select register
            0x4000..=0x5FFF => {
                self.ram_bank_index = data;
            }
            external_ram_address @ 0xA000..=0xBFFF if self.ram_enabled => {
                if (0x08..=0x0C).contains(&self.ram_bank_index) {
                    // Used for RTC registers (not implemented)
                    return;
                }
                self.ram[self.ram_bank_index as usize][(external_ram_address - 0xA000) as usize] =
                    data;
            }
            _ => (),
        }
    }
}

impl MBC for MBC3 {
    fn init(&mut self, program: &Vec<u8>) {
        let rom_select_address = 0x2000;
        for i in 0..self.rom.len() {
            self.write(rom_select_address, i as u8);
            // Figure out whether the data should be written to first or second area of rom
            let bank_offset = if i == 0 { 0 } else { 0x4000 };
            for j in 0..ROM_BANK_SIZE {
                self.init_write(
                    bank_offset + j as u16,
                    program[ROM_BANK_SIZE * i as usize + j],
                )
            }
        }

        // Set rom select register back to initial value of zero
        self.write(rom_select_address, 0);
    }
}
