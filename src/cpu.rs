use std::{fs, rc::Rc};

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
    memory: [u8; 0xFFFF + 1],
    instructions: [Option<Rc<dyn Fn(&mut Self) -> ()>>; 0xFF + 1],
}

impl CPU {
    pub fn new() -> Self {
        const INIT_INSTRUCTION: Option<Rc<dyn Fn(&mut CPU) -> ()>> = None;
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
            memory: [0; 0xFFFF + 1],
            instructions: [INIT_INSTRUCTION; 0xFF + 1],
        };

        // 8-bit LD instructions
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
            let opcode = 0b00000110 | (dest_num << 3);

            cpu.instructions[opcode as usize] = Some(Rc::new(move |cpu: &mut CPU| {
                let source = cpu.read(cpu.program_counter);
                cpu.program_counter += 1;
                let dest_option = cpu.get_register(dest_num);
                if let Some(dest) = dest_option {
                    *dest = source;
                }
            }));
        }

        // LD r, (HL)  (2 M-cycles)
        for i in 0..8 {
            let dest_num = i as u8;
            let opcode = 0b01000110 | (dest_num << 3);

            cpu.instructions[opcode as usize] = Some(Rc::new(move |cpu: &mut CPU| {
                let source = cpu.read(CPU::combine_bytes(cpu.register_h, cpu.register_l));
                let dest_option = cpu.get_register(dest_num);
                if let Some(dest) = dest_option {
                    *dest = source;
                }
            }));
        }

        // LD (HL), r  (2 M-cycles)
        for i in 0..8 {
            let source_num = i as u8;
            let opcode = 0b01110000 | source_num;

            cpu.instructions[opcode as usize] = Some(Rc::new(move |cpu: &mut CPU| {
                let source_option = cpu.get_register(source_num);
                if let Some(source_reg) = source_option {
                    let source = *source_reg;
                    cpu.write(CPU::combine_bytes(cpu.register_h, cpu.register_l), source);
                }
            }));
        }

        // LD (HL), n  (3 M-cycles)
        cpu.instructions[0b00110110] = Some(Rc::new(|cpu: &mut CPU| {
            let source = cpu.read(cpu.program_counter);
            cpu.program_counter += 1;
            cpu.write(CPU::combine_bytes(cpu.register_h, cpu.register_l), source);
        }));

        // LD A, (BC)  (2 M-cycles)
        cpu.instructions[0x0A] = Some(Rc::new(|cpu: &mut CPU| {
            let source = cpu.read(CPU::combine_bytes(cpu.register_b, cpu.register_c));
            cpu.register_a = source;
        }));

        // LD A, (DE)  (2 M-cycles)
        cpu.instructions[0x1A] = Some(Rc::new(|cpu: &mut CPU| {
            let source = cpu.read(CPU::combine_bytes(cpu.register_d, cpu.register_e));
            cpu.register_a = source;
        }));

        // LD (BC), A  (2 M-cycles)
        cpu.instructions[0x02] = Some(Rc::new(|cpu: &mut CPU| {
            cpu.write(
                CPU::combine_bytes(cpu.register_b, cpu.register_b),
                cpu.register_a,
            );
        }));

        // LD (DE), A  (2 M-cycles)
        cpu.instructions[0x12] = Some(Rc::new(|cpu: &mut CPU| {
            cpu.write(
                CPU::combine_bytes(cpu.register_d, cpu.register_e),
                cpu.register_a,
            );
        }));

        // LD A, nn  (4 M-cycles)
        cpu.instructions[0xFA] = Some(Rc::new(|cpu: &mut CPU| {
            let low = cpu.read(cpu.program_counter);
            cpu.program_counter += 1;
            let high = cpu.read(cpu.program_counter);
            cpu.program_counter += 1;
            cpu.register_a = cpu.read(CPU::combine_bytes(high, low));
        }));

        // LD nn, A  (4 M-cycles)
        cpu.instructions[0xEA] = Some(Rc::new(|cpu: &mut CPU| {
            let low = cpu.read(cpu.program_counter);
            cpu.program_counter += 1;
            let high = cpu.read(cpu.program_counter);
            cpu.program_counter += 1;
            cpu.write(CPU::combine_bytes(high, low), cpu.register_a);
        }));

        // LDH A, C  (2 M-cycles)
        cpu.instructions[0xF2] = Some(Rc::new(|cpu: &mut CPU| {
            cpu.register_a = cpu.read(CPU::combine_bytes(0xFF, cpu.register_c));
        }));

        // LDH C, A  (2 M-cycles)
        cpu.instructions[0xE2] = Some(Rc::new(|cpu: &mut CPU| {
            cpu.write(CPU::combine_bytes(0xFF, cpu.register_c), cpu.register_a);
        }));

        // LDH A, n  (3 M-cycles)
        cpu.instructions[0xF0] = Some(Rc::new(|cpu: &mut CPU| {
            let low_byte = cpu.read(cpu.program_counter);
            cpu.program_counter += 1;
            cpu.register_a = cpu.read(CPU::combine_bytes(0xFF, low_byte));
        }));

        // LDH n, A  (3 M-cycles)
        cpu.instructions[0xE0] = Some(Rc::new(|cpu: &mut CPU| {
            let low_byte = cpu.read(cpu.program_counter);
            cpu.program_counter += 1;
            cpu.write(CPU::combine_bytes(0xFF, low_byte), cpu.register_a);
        }));

        // LDI A (HL)  (2 M-cycles)
        cpu.instructions[0x2A] = Some(Rc::new(|cpu: &mut CPU| {
            let mut hl = CPU::combine_bytes(cpu.register_h, cpu.register_l);
            cpu.register_a = cpu.read(hl);
            hl += 1;
            cpu.register_h = (hl >> 8) as u8;
            cpu.register_l = hl as u8
        }));

        // LDI (HL) A  (2 M-cycles)
        cpu.instructions[0x22] = Some(Rc::new(|cpu: &mut CPU| {
            let mut hl = CPU::combine_bytes(cpu.register_h, cpu.register_l);
            cpu.write(hl, cpu.register_a);
            hl += 1;
            cpu.register_h = (hl >> 8) as u8;
            cpu.register_l = hl as u8
        }));

        // LDD A (HL)  (2 M-cycles)
        cpu.instructions[0x3A] = Some(Rc::new(|cpu: &mut CPU| {
            let mut hl = CPU::combine_bytes(cpu.register_h, cpu.register_l);
            cpu.register_a = cpu.read(hl);
            hl -= 1;
            cpu.register_h = (hl >> 8) as u8;
            cpu.register_l = hl as u8
        }));

        // LDD (HL) A  (2 M-cycles)
        cpu.instructions[0x32] = Some(Rc::new(|cpu: &mut CPU| {
            let mut hl = CPU::combine_bytes(cpu.register_h, cpu.register_l);
            cpu.write(hl, cpu.register_a);
            hl -= 1;
            cpu.register_h = (hl >> 8) as u8;
            cpu.register_l = hl as u8
        }));

        // 16-bit LD instructions
        // LD rr, nn  (3 M-cycles)
        // combined registers version
        for i in 0..3 {
            let dest_num = i as u8;
            let opcode = 0b00000001 | (dest_num << 4);

            cpu.instructions[opcode as usize] = Some(Rc::new(move |cpu: &mut CPU| {
                let source = cpu.read_u16(cpu.program_counter);
                cpu.program_counter += 2;
                let dest_option = cpu.get_register_pair(dest_num);
                if let Some(dest) = dest_option {
                    *dest.0 = (source >> 8) as u8;
                    *dest.1 = source as u8;
                }
            }));
        }
        // stack_pointer version
        cpu.instructions[0x31] = Some(Rc::new(move |cpu: &mut CPU| {
            let source = cpu.read_u16(cpu.program_counter);
            cpu.program_counter += 2;
            cpu.stack_pointer = source;
        }));

        // LD nn (SP)  (5 M-cycles)
        cpu.instructions[0x08] = Some(Rc::new(move |cpu: &mut CPU| {
            let dest = cpu.read_u16(cpu.program_counter);
            cpu.program_counter += 2;
            cpu.write_u16(dest, cpu.stack_pointer);
        }));

        // LD SP (HL)  (2 M-cycles)
        cpu.instructions[0xF9] = Some(Rc::new(move |cpu: &mut CPU| {
            cpu.stack_pointer = (cpu.register_h as u16) << 8 | cpu.register_l as u16;
        }));

        // PUSH rr  (4 M-cycles)
        for i in 0..4 {
            let source_num = i as u8;
            let opcode = 0b11000101 | (source_num << 4);

            cpu.instructions[opcode as usize] = Some(Rc::new(move |cpu: &mut CPU| {
                let source_option = cpu.get_register_pair(source_num);
                if let Some((high, low)) = source_option {
                    let source = CPU::combine_bytes(*high, *low);
                    cpu.stack_pointer -= 2;
                    cpu.write_u16(cpu.stack_pointer, source);
                }
            }));
        }

        // POP rr  (3 M-cycles)
        for i in 0..4 {
            let dest_num = i as u8;
            let opcode = 0b11000001 | (dest_num << 4);

            cpu.instructions[opcode as usize] = Some(Rc::new(move |cpu: &mut CPU| {
                let source = cpu.read_u16(cpu.stack_pointer);
                cpu.stack_pointer += 2;
                let dest_option = cpu.get_register_pair(dest_num);
                if let Some((high, low)) = dest_option {
                    *low = source as u8;
                    *high = (source >> 8) as u8;
                }
            }));
        }

        cpu
    }

    pub fn load(&mut self, rom_path: &str) -> std::io::Result<()> {
        const ROM_LIMIT: u16 = 0x8000;
        let program = fs::read(rom_path)?;
        for i in 0..program.len() {
            if i >= ROM_LIMIT as usize {
                break;
            }
            self.write(i as u16, program[i]);
        }
        Ok(())
    }

    pub fn run(&mut self) {
        const ROM_LIMIT: u16 = 0x8000;
        while self.program_counter < ROM_LIMIT {
            let opcode = self.read(self.program_counter);
            self.program_counter += 1;
            self.execute(opcode);
        }
    }

    pub fn load_and_run(&mut self, rom_path: &str) {
        let status = self.load(rom_path);
        if let Ok(_) = status {
            self.run();
        }
    }

    fn run_test(&mut self, program: Vec<u8>) {
        for (i, b) in program.iter().enumerate() {
            self.write(self.program_counter + i as u16, *b);
        }

        let initial_pc = self.program_counter as usize;
        while self.program_counter as usize <= initial_pc + program.len() {
            let opcode = self.read(self.program_counter);
            self.program_counter += 1;
            self.execute(opcode);
        }
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

    fn get_register_pair(&mut self, code: u8) -> Option<(&mut u8, &mut u8)> {
        match code {
            0 => Some((&mut self.register_b, &mut self.register_c)),
            1 => Some((&mut self.register_d, &mut self.register_e)),
            2 => Some((&mut self.register_h, &mut self.register_l)),
            3 => Some((&mut self.register_a, &mut self.register_f)),
            _ => None,
        }
    }
}

impl Memory for CPU {
    fn read(&self, address: u16) -> u8 {
        let data = self.memory[address as usize];
        //println!("Read value {:x} from {:x}", data, address);
        data
    }

    fn write(&mut self, address: u16, data: u8) {
        //println!("Wrote value {:x} to {:x}", data, address);
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
    fn two_loads() {
        let mut cpu = CPU::new();
        // LD b, a
        // LD d, a
        cpu.run_test(vec![0x47, 0x57]);
        assert_eq!(cpu.register_b, cpu.register_d);
    }

    #[test]
    fn load_immediate_args_not_used_as_opcodes() {
        let mut cpu = CPU::new();
        // LD b, 0x1A (1A is the opcode for LD a, (DE))
        // LD d, 0xF1
        cpu.run_test(vec![0x06, 0x1A, 0x16, 0xF1]);
        assert_eq!(cpu.register_a, 0x11);
        assert_eq!(cpu.register_b, 0x1A);
        assert_eq!(cpu.register_d, 0xF1);
    }

    #[test]
    fn loaded_register_not_changed() {
        let mut cpu = CPU::new();
        // LD e, c
        cpu.run_test(vec![0x59]);
        assert_eq!(cpu.register_c, 0x00);
    }

    #[test]
    fn ld_a_immediate_value() {
        let mut cpu = CPU::new();
        cpu.run_test(vec![0x3E, 0x08]);
        assert_eq!(cpu.register_a, 0x08);
    }

    #[test]
    fn ld_b_immediate_value() {
        let mut cpu = CPU::new();
        cpu.run_test(vec![0x06, 0xFF]);
        assert_eq!(cpu.register_b, 0xFF);
    }

    #[test]
    fn ld_c_immediate_value() {
        let mut cpu = CPU::new();
        cpu.run_test(vec![0x0E, 0x12]);
        assert_eq!(cpu.register_c, 0x12);
    }

    #[test]
    fn ld_d_immediate_value() {
        let mut cpu = CPU::new();
        cpu.run_test(vec![0x16, 0x00]);
        assert_eq!(cpu.register_d, 0x00);
    }

    #[test]
    fn ld_a_hl() {
        let mut cpu = CPU::new();
        cpu.run_test(vec![0b01111110]);
        assert_eq!(cpu.register_a, 0x00);
    }

    #[test]
    fn ld_hl_e() {
        let mut cpu = CPU::new();
        cpu.run_test(vec![0b01110011]);
        assert_eq!(
            cpu.read(CPU::combine_bytes(cpu.register_h, cpu.register_l)),
            0x56
        );
    }

    #[test]
    fn ld_hl_immediate() {
        let mut cpu = CPU::new();
        cpu.run_test(vec![0b00110110, 0x87]);
        assert_eq!(
            cpu.read(CPU::combine_bytes(cpu.register_h, cpu.register_l)),
            0x87
        );
    }

    #[test]
    fn ld_a_bc() {
        let mut cpu = CPU::new();
        cpu.execute(0x0A);
        let val = cpu.read(CPU::combine_bytes(cpu.register_b, cpu.register_c));
        assert_eq!(cpu.register_a, val);
    }

    #[test]
    fn ld_a_de() {
        let mut cpu = CPU::new();
        cpu.execute(0x1A);
        let val = cpu.read(CPU::combine_bytes(cpu.register_d, cpu.register_e));
        assert_eq!(cpu.register_a, val);
    }

    #[test]
    fn ld_bc_a() {
        let mut cpu = CPU::new();
        cpu.execute(0x02);
        let val = cpu.read(CPU::combine_bytes(cpu.register_b, cpu.register_c));
        assert_eq!(val, cpu.register_a);
    }

    #[test]
    fn ld_de_a() {
        let mut cpu = CPU::new();
        cpu.execute(0x12);
        let val = cpu.read(CPU::combine_bytes(cpu.register_d, cpu.register_e));
        assert_eq!(val, cpu.register_a);
    }

    #[test]
    fn ld_a_nn() {
        let mut cpu = CPU::new();
        cpu.run_test(vec![0xFA, 0x01, 0x10]);
        let val = cpu.read(CPU::combine_bytes(0x10, 0x01));
        assert_eq!(cpu.register_a, val);
    }

    #[test]
    fn ld_nn_a() {
        let mut cpu = CPU::new();
        cpu.run_test(vec![0xEA, 0x01, 0x10]);
        let val = cpu.read(CPU::combine_bytes(0x10, 0x01));
        assert_eq!(cpu.register_a, val);
    }

    #[test]
    fn multiple_ld_instructions_with_varying_args() {
        let mut cpu = CPU::new();
        // ld a, $3456
        // ld c, a
        // ld b, $78
        cpu.run_test(vec![0xFA, 0x56, 0x34, 0x4B, 0x06, 0x78]);
        assert_eq!(cpu.register_a, 0x00);
        assert_eq!(cpu.register_c, 0x56);
        assert_eq!(cpu.register_b, 0x78)
    }

    #[test]
    fn ldh_a_c() {
        let mut cpu = CPU::new();
        cpu.run_test(vec![0xF2]);
        let val = cpu.read(CPU::combine_bytes(0xFF, cpu.register_c));
        assert_eq!(cpu.register_a, val);
    }

    #[test]
    fn ldh_c_a() {
        let mut cpu = CPU::new();
        cpu.run_test(vec![0xE2]);
        let val = cpu.read(CPU::combine_bytes(0xFF, cpu.register_c));
        assert_eq!(val, 0x11);
    }

    #[test]
    fn ldh_a_n() {
        let mut cpu = CPU::new();
        cpu.run_test(vec![0xF0, 0x06]);
        let val = cpu.read(CPU::combine_bytes(0xFF, 0x06));
        assert_eq!(cpu.register_a, val);
    }

    #[test]
    fn ldh_n_a() {
        let mut cpu = CPU::new();
        cpu.run_test(vec![0xE0, 0x0A]);
        let val = cpu.read(CPU::combine_bytes(0xFF, 0x0A));
        assert_eq!(val, 0x11);
    }

    #[test]
    fn ldi_a_hl() {
        let mut cpu = CPU::new();
        let initial = CPU::combine_bytes(cpu.register_h, cpu.register_l);
        cpu.run_test(vec![0x2A]);
        let changed = CPU::combine_bytes(cpu.register_h, cpu.register_l);
        assert_eq!(cpu.register_a, 0);
        assert_eq!(changed - initial, 1);
    }

    #[test]
    fn ldi_hl_a() {
        let mut cpu = CPU::new();
        let initial = CPU::combine_bytes(cpu.register_h, cpu.register_l);
        cpu.run_test(vec![0x22]);
        let val = cpu.read(initial);
        let changed = CPU::combine_bytes(cpu.register_h, cpu.register_l);
        assert_eq!(val, 0x11);
        assert_eq!(changed - initial, 1);
    }

    #[test]
    fn ldd_a_hl() {
        let mut cpu = CPU::new();
        let initial = CPU::combine_bytes(cpu.register_h, cpu.register_l);
        cpu.run_test(vec![0x3A]);
        let changed = CPU::combine_bytes(cpu.register_h, cpu.register_l);
        assert_eq!(cpu.register_a, 0);
        assert_eq!(initial - changed, 1);
    }

    #[test]
    fn ldd_hl_a() {
        let mut cpu = CPU::new();
        let initial = CPU::combine_bytes(cpu.register_h, cpu.register_l);
        cpu.run_test(vec![0x32]);
        let val = cpu.read(initial);
        let changed = CPU::combine_bytes(cpu.register_h, cpu.register_l);
        assert_eq!(val, 0x11);
        assert_eq!(initial - changed, 1);
    }

    #[test]
    fn ld_bc_immediate_u16() {
        let mut cpu = CPU::new();
        cpu.run_test(vec![0x01, 0x08, 0x05]);
        assert_eq!(cpu.register_b, 0x05);
        assert_eq!(cpu.register_c, 0x08);
    }

    #[test]
    fn ld_de_immediate_u16() {
        let mut cpu = CPU::new();
        cpu.run_test(vec![0x11, 0x08, 0x05]);
        assert_eq!(cpu.register_d, 0x05);
        assert_eq!(cpu.register_e, 0x08);
    }

    #[test]
    fn ld_hl_immediate_u16() {
        let mut cpu = CPU::new();
        cpu.run_test(vec![0x21, 0x08, 0x05]);
        assert_eq!(cpu.register_h, 0x05);
        assert_eq!(cpu.register_l, 0x08);
    }

    #[test]
    fn ld_sp_immediate_u16() {
        let mut cpu = CPU::new();
        cpu.run_test(vec![0x31, 0x08, 0x05]);
        assert_eq!(cpu.stack_pointer, 0x0508);
    }

    #[test]
    fn ld_nn_sp_u16() {
        let mut cpu = CPU::new();
        cpu.run_test(vec![0x08, 0x0F, 0x02]);
        assert_eq!(cpu.read_u16(0x020f), 0xFFFE);
    }

    #[test]
    fn ld_sp_hl_u16() {
        let mut cpu = CPU::new();
        cpu.run_test(vec![0xF9]);
        assert_eq!(
            cpu.stack_pointer,
            CPU::combine_bytes(cpu.register_h, cpu.register_l)
        );
    }

    #[test]
    fn push_bc() {
        let mut cpu = CPU::new();
        let initial = cpu.stack_pointer;
        cpu.run_test(vec![0xC5]);
        let changed = cpu.stack_pointer;
        assert_eq!(initial - changed, 2);
        assert_eq!(cpu.read(cpu.stack_pointer), cpu.register_c);
    }

    #[test]
    #[should_panic]
    fn pop_without_pushing_first() {
        let mut cpu = CPU::new();
        cpu.run_test(vec![0xF1]);
    }

    #[test]
    fn push_bc_pop_af() {
        let mut cpu = CPU::new();
        let initial = cpu.stack_pointer;
        cpu.run_test(vec![0xC5, 0xF1]);
        let changed = cpu.stack_pointer;
        assert_eq!(initial, changed);
        assert_eq!(
            (cpu.register_b, cpu.register_c),
            (cpu.register_a, cpu.register_f)
        );
    }

    #[test]
    fn multiple_pushes_multiple_pops() {
        let mut cpu = CPU::new();
        let initial = cpu.stack_pointer;
        cpu.run_test(vec![0xC5, 0xD5, 0xE5, 0xC1, 0xC1]);
        let changed = cpu.stack_pointer;
        assert_eq!(initial - changed, 2);
        assert_eq!(
            (cpu.register_b, cpu.register_c),
            (cpu.register_d, cpu.register_e)
        );
    }
}
