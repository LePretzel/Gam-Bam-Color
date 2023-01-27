use crate::{mbc::MBC, memory::Memory};

const ROM_BANK_SIZE: usize = 0x4000;
const RAM_BANK_SIZE: usize = 0x2000;

pub struct MBC5 {
    rom: Vec<[u8; ROM_BANK_SIZE]>,
    ram: Vec<[u8; RAM_BANK_SIZE]>,
    ram_enabled: bool,
    lower_rom_bank_index: u8,
    upper_rom_bank_bit: bool,
    ram_bank_index: u8,
}

impl MBC5 {
    pub fn new(rom_banks: u8, ram_banks: u8) -> Self {
        let mut mbc = MBC5 {
            rom: Vec::with_capacity(rom_banks as usize),
            ram: Vec::with_capacity(ram_banks as usize),
            ram_enabled: false,
            lower_rom_bank_index: 0,
            upper_rom_bank_bit: false,
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
                let high_bit = if self.upper_rom_bank_bit {
                    0b1_0000_0000
                } else {
                    0
                };
                self.rom[high_bit | self.lower_rom_bank_index as usize]
                    [(other_rom_banks_address - 0x4000) as usize] = data;
            }
            external_ram_address @ 0xA000..=0xBFFF if self.ram_enabled => {
                self.ram[self.ram_bank_index as usize][(external_ram_address - 0xA000) as usize] =
                    data;
            }
            _ => (),
        }
    }
}

impl Memory for MBC5 {
    fn read(&self, address: u16) -> u8 {
        match address {
            rom_bank_one_address @ 0x0000..=0x3FFF => self.rom[0][rom_bank_one_address as usize],
            other_rom_banks_address @ 0x4000..=0x7FFF => {
                let high_bit = if self.upper_rom_bank_bit {
                    0b1_0000_0000
                } else {
                    0
                };
                self.rom[high_bit | self.lower_rom_bank_index as usize]
                    [(other_rom_banks_address - 0x4000) as usize]
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
            // Rom bank select register for lower 8 bits
            0x2000..=0x2FFF => {
                let mask = if data as usize > self.rom.len() {
                    // Cut off bits if the rom bank would be too high for the cartridge
                    self.rom.len() as u8 - 1
                } else {
                    // Else use all bits
                    0xFF
                };
                self.lower_rom_bank_index = mask & data;
            }
            // Rom bank select register for highest bit
            0x3000..=0x3FFF => {
                self.upper_rom_bank_bit = if data == 0 { false } else { true };
            }
            // Ram bank select register
            0x4000..=0x5FFF => {
                self.ram_bank_index = data;
            }
            external_ram_address @ 0xA000..=0xBFFF if self.ram_enabled => {
                self.ram[self.ram_bank_index as usize][(external_ram_address - 0xA000) as usize] =
                    data;
            }
            _ => (),
        }
    }
}

impl MBC for MBC5 {
    fn init(&mut self, program: &Vec<u8>) {
        let lower_rom_select_address = 0x2000;
        let upper_rom_select_address = 0x3000;
        for i in 0..self.rom.len() {
            self.write(lower_rom_select_address, i as u8);
            if i == 0x100 {
                // Used to initialize the banks past 0xFF
                self.write(upper_rom_select_address, 1);
            }

            let other_rom_banks_start = 0x4000;
            for j in 0..ROM_BANK_SIZE {
                self.init_write(
                    other_rom_banks_start + j as u16,
                    program[ROM_BANK_SIZE * i as usize + j],
                )
            }
        }

        // Set rom select registers back to initial values
        self.write(lower_rom_select_address, 0);
        self.upper_rom_bank_bit = false;
    }
}
