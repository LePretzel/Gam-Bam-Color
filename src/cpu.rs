use std::rc::Rc;

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
    instructions: [Option<Rc<dyn Fn(&mut Self) -> ()>>; 0xFF],
}

impl CPU {
    pub fn new() -> Self {
        const init_instruction: Option<Rc<dyn Fn(&mut CPU) -> ()>> = None;
        let mut cpu = CPU {
            register_a: 0x11,
            register_f: 0x80,
            register_b: 0x00,
            register_c: 0x00,
            register_d: 0xFF,
            register_e: 0x56,
            register_h: 0x00,
            register_l: 0x0D,
            stack_pointer: 0xFFFE,
            program_counter: 0x0100,
            memory: [0; 0xFFFF],
            instructions: [init_instruction; 0xFF],
        };

        // LD instructions
        // LD r, r'  (1 M-cycle)
        for i in 0..8 {
            for j in 0..8 {
                let source_num = j as u8;
                let dest_num = i as u8;
                let opcode: u8 = 0b01000000 | source_num | (dest_num << 3);

                cpu.instructions[opcode as usize] = Some(Rc::new(move |cpu: &mut CPU| {
                    let source_option = cpu.get_register(source_num);
                    if source_option.is_some() {
                        let source = *source_option.unwrap();
                        let dest_option = cpu.get_register(dest_num);
                        if dest_option.is_some() {
                            let dest = dest_option.unwrap();
                            *dest = source;
                        }
                    }
                }));
            }
        }

        // LD r, n  (2 M-cycles)
        for i in 0..8 {
            let dest_num = i as u8;
            let opcode: u8 = 0b00000110 | (dest_num << 3);

            cpu.instructions[opcode as usize] = Some(Rc::new(move |cpu: &mut CPU| {
                let source = cpu.read(cpu.stack_pointer);
                let dest_option = cpu.get_register(dest_num);
                if let Some(dest) = dest_option {
                    *dest = source;
                }
            }));
        }

        // LD A, (DE)  (2 M-cycles)
        cpu.instructions[0x1A] = Some(Rc::new(|cpu: &mut CPU| {
            let source = cpu.read(CPU::combine_bytes(cpu.register_d, cpu.register_e));
            cpu.register_a = source;
        }));
        cpu
    }

    fn execute(&mut self, opcode: u8) {
        if self.instructions[opcode as usize].is_none() {
            return;
        }
        let inst = self.instructions[opcode as usize].as_ref().unwrap().clone();
        inst(self);
    }

    fn combine_bytes(high: u8, low: u8) -> u16 {
        ((high as u16) << 8) | low as u16
    }

    fn get_register(&mut self, code: u8) -> Option<&mut u8> {
        match code {
            0 => Some(&mut self.register_b),
            1 => Some(&mut self.register_c),
            2 => Some(&mut self.register_d),
            3 => Some(&mut self.register_e),
            4 => Some(&mut self.register_h),
            5 => Some(&mut self.register_l),
            7 => Some(&mut self.register_a),
            _ => None,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ld_a_b() {
        let mut cpu = CPU::new();
        cpu.execute(0b01111000);
        assert_eq!(cpu.register_a, 0x00);
    }

    #[test]
    fn ld_a_d() {
        let mut cpu = CPU::new();
        cpu.execute(0b01111010);
        assert_eq!(cpu.register_a, 0xFF);
    }

    #[test]
    fn ld_b_l() {
        let mut cpu = CPU::new();
        cpu.execute(0x45);
        assert_eq!(cpu.register_b, 0x0D);
    }

    #[test]
    fn ld_c_a() {
        let mut cpu = CPU::new();
        cpu.execute(0x4f);
        assert_eq!(cpu.register_c, 0x11);
    }

    #[test]
    fn ld_d_h() {
        let mut cpu = CPU::new();
        cpu.execute(0x54);
        assert_eq!(cpu.register_d, 0x00);
    }

    #[test]
    fn ld_e_c() {
        let mut cpu = CPU::new();
        cpu.execute(0x59);
        assert_eq!(cpu.register_e, 0x00);
    }

    #[test]
    fn ld_h_e() {
        let mut cpu = CPU::new();
        cpu.execute(0x63);
        assert_eq!(cpu.register_h, 0x56);
    }

    #[test]
    fn ld_l_a() {
        let mut cpu = CPU::new();
        cpu.execute(0x6F);
        assert_eq!(cpu.register_l, 0x11);
    }

    #[test]
    fn ld_a_de() {
        let mut cpu = CPU::new();
        cpu.execute(0x1A);
        let val = cpu.read(CPU::combine_bytes(cpu.register_d, cpu.register_e));
        assert_eq!(cpu.register_a, val);
    }
}
