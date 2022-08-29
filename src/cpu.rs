use crate::memory::Memory;

pub struct CPU {
    // Register pairs
    register_a: u8, // Accumulator
    register_f: u8, // Flags

    register_b: u8, // General purpose
    register_c: u8, // Counter

    register_d: u8, // General purpose
    register_e: u8, // General pupose

    register_h: u8, // High pointer
    register_l: u8, // Low pointer

    stack_pointer: u16,
    program_counter: u16,
    memory: [u8; 0xFFFF],
}

impl CPU {
    pub fn new() -> Self {
        CPU {
            register_a: 0x11,
            register_f: 0x80,
            register_b: 0x00,
            register_c: 0x00,
            register_d: 0xFF,
            register_e: 0x56,
            register_h: 0x00,
            register_l: 0x0D,
            stack_pointer: 0x0100,
            program_counter: 0xFFFE,
            memory: [0; 0xFFFF],
        }
    }
}

impl Memory for CPU {
    fn read(&self, address: u16) -> u8 {
        self.memory[address as usize]
    }

    fn write(&mut self, address: u16, data: u8) {
        self.memory[address as usize] = data;
    }

    fn read_u16(&self, address: u16) -> u16 {
        // The low byte is first because the GB CPU uses Little-Endian addressing
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
