use std::{fs, num::Wrapping, rc::Rc};

use crate::cpu::Operand::{Indirect, Register};
use crate::memory::Memory;

enum Operand {
    Register(u8),
    Indirect(u16),
}

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

        // LD A, (nn)  (4 M-cycles)
        cpu.instructions[0xFA] = Some(Rc::new(|cpu: &mut CPU| {
            let low = cpu.read(cpu.program_counter);
            cpu.program_counter += 1;
            let high = cpu.read(cpu.program_counter);
            cpu.program_counter += 1;
            cpu.register_a = cpu.read(CPU::combine_bytes(high, low));
        }));

        // LD (nn), A  (4 M-cycles)
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

        // LD nn SP  (5 M-cycles)
        cpu.instructions[0x08] = Some(Rc::new(move |cpu: &mut CPU| {
            let dest = cpu.read_u16(cpu.program_counter);
            cpu.program_counter += 2;
            cpu.write_u16(dest, cpu.stack_pointer);
        }));

        // LD SP HL  (2 M-cycles)
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

        // 8-bit arithmetic/logic instructions
        // ADD A, r  (1 M-cycles)
        for i in 0..8 {
            let register_num = i as u8;
            let opcode = 0b10000000 | register_num;

            cpu.instructions[opcode as usize] = Some(Rc::new(move |cpu: &mut CPU| {
                let register_option = cpu.get_register(register_num);
                if let Some(reg) = register_option {
                    let reg_value = *reg;
                    cpu.update_flags_add(cpu.register_a, reg_value);
                    let mut sum = Wrapping(reg_value);
                    sum += cpu.register_a;
                    cpu.register_a = sum.0;
                }
            }));
        }

        // ADD A, n  (2 M-cycles)
        cpu.instructions[0xC6] = Some(Rc::new(move |cpu: &mut CPU| {
            let arg = cpu.read(cpu.program_counter);
            cpu.program_counter += 1;
            cpu.update_flags_add(cpu.register_a, arg);
            let mut sum = Wrapping(cpu.register_a);
            sum += arg;
            cpu.register_a = sum.0;
        }));

        // ADD A, (HL)  (2 M-cycles)
        cpu.instructions[0x86] = Some(Rc::new(move |cpu: &mut CPU| {
            let arg = cpu.read(CPU::combine_bytes(cpu.register_h, cpu.register_l));
            cpu.update_flags_add(cpu.register_a, arg);
            let mut sum = Wrapping(cpu.register_a);
            sum += arg;
            cpu.register_a = sum.0;
        }));

        // ADC A, r  (1 M-cycles)
        for i in 0..8 {
            let register_num = i as u8;
            let opcode = 0b10001000 | register_num;

            cpu.instructions[opcode as usize] = Some(Rc::new(move |cpu: &mut CPU| {
                let register_option = cpu.get_register(register_num);
                if let Some(reg) = register_option {
                    let reg_value = *reg;
                    let carry_bit = (cpu.register_f & 0b00010000) >> 4;
                    cpu.update_flags_add(cpu.register_a, reg_value + carry_bit);
                    let mut sum = Wrapping(reg_value);
                    sum += cpu.register_a;
                    sum += carry_bit;
                    cpu.register_a = sum.0;
                }
            }));
        }

        // ADC A, n  (2 M-cycles)
        cpu.instructions[0xCE] = Some(Rc::new(move |cpu: &mut CPU| {
            let arg = cpu.read(cpu.program_counter);
            cpu.program_counter += 1;
            let carry_bit = (cpu.register_f & 0b00010000) >> 4;
            cpu.update_flags_add(cpu.register_a, arg + carry_bit);
            let mut sum = Wrapping(cpu.register_a);
            sum += arg;
            sum += carry_bit;
            cpu.register_a = sum.0;
        }));

        // ADC A, (HL)  (2 M-cycles)
        cpu.instructions[0x8E] = Some(Rc::new(move |cpu: &mut CPU| {
            let arg = cpu.read(CPU::combine_bytes(cpu.register_h, cpu.register_l));
            let carry_bit = (cpu.register_f & 0b00010000) >> 4;
            cpu.update_flags_add(cpu.register_a, arg + carry_bit);
            let mut sum = Wrapping(cpu.register_a);
            sum += arg;
            sum += carry_bit;
            cpu.register_a = sum.0;
        }));

        // SUB A, r  (1 M-cycles)
        for i in 0..8 {
            let register_num = i as u8;
            let opcode = 0b10010000 | register_num;

            cpu.instructions[opcode as usize] = Some(Rc::new(move |cpu: &mut CPU| {
                let register_option = cpu.get_register(register_num);
                if let Some(reg) = register_option {
                    let reg_value = CPU::negate(*reg);
                    cpu.update_flags_sub(cpu.register_a, reg_value);
                    let mut sum = Wrapping(reg_value);
                    sum += cpu.register_a;
                    cpu.register_a = sum.0;
                }
            }));
        }

        // SUB A, n  (2 M-cycles)
        cpu.instructions[0xD6] = Some(Rc::new(move |cpu: &mut CPU| {
            let arg = CPU::negate(cpu.read(cpu.program_counter));
            cpu.program_counter += 1;
            cpu.update_flags_sub(cpu.register_a, arg);
            let mut sum = Wrapping(cpu.register_a);
            sum += arg;
            cpu.register_a = sum.0;
        }));

        // SUB A, (HL)  (2 M-cycles)
        cpu.instructions[0x96] = Some(Rc::new(move |cpu: &mut CPU| {
            let arg = CPU::negate(cpu.read(CPU::combine_bytes(cpu.register_h, cpu.register_l)));
            cpu.update_flags_sub(cpu.register_a, arg);
            let mut sum = Wrapping(cpu.register_a);
            sum += arg;
            cpu.register_a = sum.0;
        }));

        // SBC A, r  (1 M-cycles)
        for i in 0..8 {
            let register_num = i as u8;
            let opcode = 0b10011000 | register_num;

            cpu.instructions[opcode as usize] = Some(Rc::new(move |cpu: &mut CPU| {
                let register_option = cpu.get_register(register_num);
                if let Some(reg) = register_option {
                    let reg_value = CPU::negate(*reg);
                    let carry_bit = (cpu.register_f & 0b00010000) >> 4;
                    cpu.update_flags_sub(cpu.register_a, reg_value);
                    let mut sum = Wrapping(reg_value);
                    sum += cpu.register_a;
                    sum -= carry_bit;
                    cpu.register_a = sum.0;
                }
            }));
        }

        // SBC A, n  (2 M-cycles)
        cpu.instructions[0xDE] = Some(Rc::new(move |cpu: &mut CPU| {
            let arg = CPU::negate(cpu.read(cpu.program_counter));
            let carry_bit = (cpu.register_f & 0b00010000) >> 4;
            cpu.program_counter += 1;
            cpu.update_flags_sub(cpu.register_a, arg);
            let mut sum = Wrapping(cpu.register_a);
            sum += arg;
            sum -= carry_bit;
            cpu.register_a = sum.0;
        }));

        // SBC A, (HL)  (2 M-cycles)
        cpu.instructions[0x9E] = Some(Rc::new(move |cpu: &mut CPU| {
            let arg = CPU::negate(cpu.read(CPU::combine_bytes(cpu.register_h, cpu.register_l)));
            let carry_bit = (cpu.register_f & 0b00010000) >> 4;
            cpu.update_flags_sub(cpu.register_a, arg);
            let mut sum = Wrapping(cpu.register_a);
            sum += arg;
            sum -= carry_bit;
            cpu.register_a = sum.0;
        }));

        // AND A, r  (1 M-cycles)
        for i in 0..8 {
            let register_num = i as u8;
            let opcode = 0b10100000 | register_num;

            cpu.instructions[opcode as usize] = Some(Rc::new(move |cpu: &mut CPU| {
                let register_option = cpu.get_register(register_num);
                if let Some(reg) = register_option {
                    let register_value = *reg;
                    cpu.register_a = cpu.register_a & register_value;
                    cpu.register_f = 0b00100000 | ((cpu.register_a == 0) as u8) << 7;
                }
            }));
        }

        // AND A, n  (2 M-cycles)
        cpu.instructions[0xE6] = Some(Rc::new(move |cpu: &mut CPU| {
            let arg = cpu.read(cpu.program_counter);
            cpu.program_counter += 1;
            cpu.register_a = cpu.register_a & arg;
            cpu.register_f = 0b00100000 | ((cpu.register_a == 0) as u8) << 7;
        }));

        // AND A, (HL)  (2 M-cycles)
        cpu.instructions[0xA6] = Some(Rc::new(move |cpu: &mut CPU| {
            let arg = cpu.read(CPU::combine_bytes(cpu.register_h, cpu.register_l));
            cpu.register_a = cpu.register_a & arg;
            cpu.register_f = 0b00100000 | ((cpu.register_a == 0) as u8) << 7;
        }));

        // XOR A, r  (1 M-cycles)
        for i in 0..8 {
            let register_num = i as u8;
            let opcode = 0b10101000 | register_num;

            cpu.instructions[opcode as usize] = Some(Rc::new(move |cpu: &mut CPU| {
                let register_option = cpu.get_register(register_num);
                if let Some(reg) = register_option {
                    let register_value = *reg;
                    cpu.register_a = cpu.register_a ^ register_value;
                    cpu.register_f = ((cpu.register_a == 0) as u8) << 7;
                }
            }));
        }

        // XOR A, n  (2 M-cycles)
        cpu.instructions[0xEE] = Some(Rc::new(move |cpu: &mut CPU| {
            let arg = cpu.read(cpu.program_counter);
            cpu.program_counter += 1;
            cpu.register_a = cpu.register_a ^ arg;
            cpu.register_f = ((cpu.register_a == 0) as u8) << 7;
        }));

        // XOR A, (HL)  (2 M-cycles)
        cpu.instructions[0xAE] = Some(Rc::new(move |cpu: &mut CPU| {
            let arg = cpu.read(CPU::combine_bytes(cpu.register_h, cpu.register_l));
            cpu.register_a = cpu.register_a ^ arg;
            cpu.register_f = ((cpu.register_a == 0) as u8) << 7;
        }));

        // OR A, r  (1 M-cycles)
        for i in 0..8 {
            let register_num = i as u8;
            let opcode = 0b10110000 | register_num;

            cpu.instructions[opcode as usize] = Some(Rc::new(move |cpu: &mut CPU| {
                let register_option = cpu.get_register(register_num);
                if let Some(reg) = register_option {
                    let register_value = *reg;
                    cpu.register_a = cpu.register_a | register_value;
                    cpu.register_f = ((cpu.register_a == 0) as u8) << 7;
                }
            }));
        }

        // OR A, n  (2 M-cycles)
        cpu.instructions[0xF6] = Some(Rc::new(move |cpu: &mut CPU| {
            let arg = cpu.read(cpu.program_counter);
            cpu.program_counter += 1;
            cpu.register_a = cpu.register_a | arg;
            cpu.register_f = ((cpu.register_a == 0) as u8) << 7;
        }));

        // OR A, (HL)  (2 M-cycles)
        cpu.instructions[0xB6] = Some(Rc::new(move |cpu: &mut CPU| {
            let arg = cpu.read(CPU::combine_bytes(cpu.register_h, cpu.register_l));
            cpu.register_a = cpu.register_a | arg;
            cpu.register_f = ((cpu.register_a == 0) as u8) << 7;
        }));

        // CP A, r  (1 M-cycles)
        for i in 0..8 {
            let register_num = i as u8;
            let opcode = 0b10111000 | register_num;

            cpu.instructions[opcode as usize] = Some(Rc::new(move |cpu: &mut CPU| {
                let register_option = cpu.get_register(register_num);
                if let Some(reg) = register_option {
                    let reg_value = CPU::negate(*reg);
                    cpu.update_flags_sub(cpu.register_a, reg_value);
                }
            }));
        }

        // CP A, n  (2 M-cycles)
        cpu.instructions[0xFE] = Some(Rc::new(move |cpu: &mut CPU| {
            let arg = CPU::negate(cpu.read(cpu.program_counter));
            cpu.program_counter += 1;
            cpu.update_flags_sub(cpu.register_a, arg);
        }));

        // CP A, (HL)  (2 M-cycles)
        cpu.instructions[0xBE] = Some(Rc::new(move |cpu: &mut CPU| {
            let arg = CPU::negate(cpu.read(CPU::combine_bytes(cpu.register_h, cpu.register_l)));
            cpu.update_flags_sub(cpu.register_a, arg);
        }));

        // INC r  (1 M-cycles)
        for i in 0..8 {
            let register_num = i as u8;
            let opcode = 0b00000100 | register_num << 3;

            cpu.instructions[opcode as usize] = Some(Rc::new(move |cpu: &mut CPU| {
                let initial_carry_bit = 0b00010000 & cpu.register_f;
                let register_option = cpu.get_register(register_num);
                if let Some(reg) = register_option {
                    let reg_value = *reg;
                    let mut sum = Wrapping(reg_value);
                    sum += 1;
                    *reg = sum.0;
                    cpu.update_flags_add(reg_value, 1);
                    cpu.register_f = (cpu.register_f & 0b11101111) | initial_carry_bit;
                }
            }));
        }

        // INC (HL)  (3 M-cycles)
        cpu.instructions[0x34] = Some(Rc::new(move |cpu: &mut CPU| {
            let initial_value = cpu.read(CPU::combine_bytes(cpu.register_h, cpu.register_l));
            let initial_carry_bit = 0b00010000 & cpu.register_f;
            let mut sum = Wrapping(initial_value);
            sum += 1;
            cpu.write(CPU::combine_bytes(cpu.register_h, cpu.register_l), sum.0);
            cpu.update_flags_add(cpu.register_a, 1);
            cpu.register_f = (cpu.register_f & 0b11101111) | initial_carry_bit;
        }));

        // DEC r  (1 M-cycles)
        for i in 0..8 {
            let register_num = i as u8;
            let opcode = 0b00000101 | register_num << 3;

            cpu.instructions[opcode as usize] = Some(Rc::new(move |cpu: &mut CPU| {
                let initial_carry_bit = 0b00010000 & cpu.register_f;
                let register_option = cpu.get_register(register_num);
                if let Some(reg) = register_option {
                    let reg_value = *reg;
                    let mut sum = Wrapping(reg_value);
                    sum -= 1;
                    *reg = sum.0;
                    cpu.update_flags_sub(cpu.register_a, 1);
                    cpu.register_f = (cpu.register_f & 0b11101111) | initial_carry_bit;
                }
            }));
        }

        // DEC  (HL)  (3 M-cycles)
        cpu.instructions[0x35] = Some(Rc::new(move |cpu: &mut CPU| {
            let initial_value = cpu.read(CPU::combine_bytes(cpu.register_h, cpu.register_l));
            let initial_carry_bit = 0b00010000 & cpu.register_f;
            let mut sum = Wrapping(initial_value);
            sum -= 1;
            cpu.write(CPU::combine_bytes(cpu.register_h, cpu.register_l), sum.0);
            cpu.update_flags_sub(cpu.register_a, 1);
            cpu.register_f = (cpu.register_f & 0b11101111) | initial_carry_bit;
        }));

        // DAA  (1 M-cycles)
        cpu.instructions[0x27] = Some(Rc::new(move |cpu: &mut CPU| {
            let low = 0b00001111 & cpu.register_a;
            let half_carry_flag = (0b00100000 & cpu.register_f) >> 5;
            let mut sum = Wrapping(cpu.register_a);
            if low > 9 || half_carry_flag == 1 {
                sum += 0x06;
            }
            let high = sum >> 4;
            let carry_flag = (0b00010000 & cpu.register_f) >> 4;
            cpu.register_f = cpu.register_f & 0b11001111;
            if high.0 > 9 || carry_flag == 1 {
                sum += 0x60;
                cpu.register_f = cpu.register_f | 0b00010000;
            }
            cpu.register_a = sum.0;
            if cpu.register_a == 0 {
                cpu.register_f = cpu.register_f | 0b10000000;
            } else {
                cpu.register_f = cpu.register_f & 0b01111111;
            }
        }));

        // CPL  (1 M-cycles)
        cpu.instructions[0x2F] = Some(Rc::new(move |cpu: &mut CPU| {
            cpu.register_a = cpu.register_a ^ 0xFF;
            cpu.register_f = cpu.register_f | 0b01100000;
        }));

        // 16-bit arithmetic/logic instructions
        // ADD Hl, rr  (2 M-cycles)
        // Combined registers version
        for i in 0..3 {
            let register_num = i as u8;
            let opcode = 0b00001001 | (register_num << 4);

            cpu.instructions[opcode as usize] = Some(Rc::new(move |cpu: &mut CPU| {
                let register_option = cpu.get_register_pair(register_num);
                if let Some((high_reg, low_reg)) = register_option {
                    let (high_value, low_value) = (*high_reg, *low_reg);
                    cpu.register_f = cpu.register_f & 0b10111111;
                    let mut sum = Wrapping(CPU::combine_bytes(cpu.register_h, cpu.register_l));
                    cpu.update_hc_flags_add_u16(sum.0, CPU::combine_bytes(high_value, low_value));
                    sum += CPU::combine_bytes(high_value, low_value);
                    cpu.register_h = (sum.0 >> 8) as u8;
                    cpu.register_l = sum.0 as u8;
                }
            }));
        }
        // Stack pointer version
        cpu.instructions[0x39] = Some(Rc::new(move |cpu: &mut CPU| {
            let high_value = (cpu.stack_pointer >> 8) as u8;
            let low_value = cpu.stack_pointer as u8;
            cpu.register_f = cpu.register_f & 0b10111111;
            let mut sum = Wrapping(CPU::combine_bytes(cpu.register_h, cpu.register_l));
            cpu.update_hc_flags_add_u16(sum.0, CPU::combine_bytes(high_value, low_value));
            sum += CPU::combine_bytes(high_value, low_value);
            cpu.register_h = (sum.0 >> 8) as u8;
            cpu.register_l = sum.0 as u8;
        }));

        // INC rr  (2 M-cycles)
        // Combined registers_version
        for i in 0..3 {
            let register_num = i as u8;
            let opcode = 0b00000011 | register_num << 4;

            cpu.instructions[opcode as usize] = Some(Rc::new(move |cpu: &mut CPU| {
                let register_option = cpu.get_register_pair(register_num);
                if let Some((high_reg, low_reg)) = register_option {
                    let (high_value, low_value) = (*high_reg, *low_reg);
                    let mut sum = Wrapping(CPU::combine_bytes(high_value, low_value));
                    sum += 1;
                    *high_reg = (sum.0 >> 8) as u8;
                    *low_reg = sum.0 as u8;
                }
            }));
        }
        // Stack pointer version
        cpu.instructions[0x33 as usize] = Some(Rc::new(move |cpu: &mut CPU| {
            let mut sum = Wrapping(cpu.stack_pointer);
            sum += 1;
            cpu.stack_pointer = sum.0;
        }));

        // DEC rr  (2 M-cycles)
        // Combined registers_version
        for i in 0..3 {
            let register_num = i as u8;
            let opcode = 0b00001011 | register_num << 4;

            cpu.instructions[opcode as usize] = Some(Rc::new(move |cpu: &mut CPU| {
                let register_option = cpu.get_register_pair(register_num);
                if let Some((high_reg, low_reg)) = register_option {
                    let (high_value, low_value) = (*high_reg, *low_reg);
                    let mut sum = Wrapping(CPU::combine_bytes(high_value, low_value));
                    sum -= 1;
                    *high_reg = (sum.0 >> 8) as u8;
                    *low_reg = sum.0 as u8;
                }
            }));
        }
        // Stack pointer version
        cpu.instructions[0x3B as usize] = Some(Rc::new(move |cpu: &mut CPU| {
            let mut sum = Wrapping(cpu.stack_pointer);
            sum -= 1;
            cpu.stack_pointer = sum.0;
        }));

        // ADD SP, dd  (4 M-cycles)
        cpu.instructions[0xE8 as usize] = Some(Rc::new(move |cpu: &mut CPU| {
            let arg = cpu.read(cpu.program_counter);
            cpu.program_counter += 1;
            let mut sum = Wrapping(cpu.stack_pointer);
            sum += arg as u16;
            cpu.stack_pointer = sum.0;
            cpu.update_flags_add(cpu.program_counter as u8, arg);
            cpu.register_f = cpu.register_f & 0b00111111;
        }));

        // LD HL, SP + dd  (3 M-cycles)
        cpu.instructions[0xF8 as usize] = Some(Rc::new(move |cpu: &mut CPU| {
            let arg = cpu.read(cpu.program_counter);
            cpu.program_counter += 1;
            let mut sum = Wrapping(cpu.stack_pointer);
            sum += arg as u16;
            cpu.register_h = (sum.0 >> 8) as u8;
            cpu.register_l = sum.0 as u8;
            cpu.update_flags_add(cpu.program_counter as u8, arg);
            cpu.register_f = cpu.register_f & 0b00111111;
        }));

        // Rotate and shift instructions
        // RLCA  (1 M-cycles)
        cpu.instructions[0x07 as usize] = Some(Rc::new(move |cpu: &mut CPU| {
            cpu.rlc(Register(7));
            cpu.register_f = cpu.register_f & 0b01111111;
        }));

        // RLA  (1 M-cycles)
        cpu.instructions[0x17 as usize] = Some(Rc::new(move |cpu: &mut CPU| {
            cpu.rl(Register(7), cpu.get_carry_bit());
            cpu.register_f = cpu.register_f & 0b01111111;
        }));

        // RRCA  (1 M-cycles)
        cpu.instructions[0x0F as usize] = Some(Rc::new(move |cpu: &mut CPU| {
            cpu.rrc(Register(7));
            cpu.register_f = cpu.register_f & 0b01111111;
        }));

        // RRA  (1 M-cycles)
        cpu.instructions[0x1F as usize] = Some(Rc::new(move |cpu: &mut CPU| {
            cpu.rr(Register(7), cpu.get_carry_bit());
            cpu.register_f = cpu.register_f & 0b01111111;
        }));

        // All 0xCB instructions
        cpu.instructions[0xCB as usize] = Some(Rc::new(move |cpu: &mut CPU| {
            let arg = cpu.read(cpu.program_counter);
            cpu.program_counter += 1;
            let arg_high_nibble = (arg & 0b11110000) >> 4;
            let arg_low_nibble = arg & 0b00001111;

            match arg_high_nibble {
                0 => match arg_low_nibble {
                    6 => {
                        // RLC (HL)  (4 M-cycles)
                        cpu.rlc(Indirect(CPU::combine_bytes(cpu.register_h, cpu.register_l)));
                    }
                    0xE => {
                        // RRC (HL)  (4 M-cycles)
                        cpu.rrc(Indirect(CPU::combine_bytes(cpu.register_h, cpu.register_l)));
                    }
                    // RLC r  (2 M-cycles)
                    reg_num @ 0..=7 => cpu.rlc(Register(reg_num)),

                    // RRC r  (2 M-cycles)
                    reg_num @ 8..=0xF => cpu.rrc(Register(reg_num - 8)),
                    _ => (),
                },
                1 => match arg_low_nibble {
                    6 => {
                        // RL (HL)  (4 M-cycles)
                        cpu.rl(
                            Indirect(CPU::combine_bytes(cpu.register_h, cpu.register_l)),
                            cpu.get_carry_bit(),
                        );
                    }
                    0xE => {
                        // RR (HL)  (4 M-cycles)
                        cpu.rr(
                            Indirect(CPU::combine_bytes(cpu.register_h, cpu.register_l)),
                            cpu.get_carry_bit(),
                        );
                    }
                    // RL r  (2 M-cycles)
                    reg_num @ 0..=7 => cpu.rl(Register(reg_num), cpu.get_carry_bit()),

                    // RR r  (2 M-cycles)
                    reg_num @ 8..=0xF => cpu.rr(Register(reg_num - 8), cpu.get_carry_bit()),
                    _ => (),
                },
                2 => match arg_low_nibble {
                    6 => {
                        // SLA (HL)  (4 M-cycles)
                        cpu.sla(Indirect(CPU::combine_bytes(cpu.register_h, cpu.register_l)));
                    }
                    0xE => {
                        // SRA (HL)  (4 M-cycles)
                        cpu.sra(Indirect(CPU::combine_bytes(cpu.register_h, cpu.register_l)));
                    }
                    // SLA r  (2 M-cycles)
                    reg_num @ 0..=7 => cpu.sla(Register(reg_num)),
                    // SRA r  (2 M-cycles)
                    reg_num @ 8..=0xF => cpu.sra(Register(reg_num - 8)),
                    _ => (),
                },
                3 => match arg_low_nibble {
                    6 => {
                        // SWAP (HL)  (4 M-cycles)
                        cpu.swap(Indirect(CPU::combine_bytes(cpu.register_h, cpu.register_l)));
                    }
                    0xE => {
                        // SRL (HL)  (4 M-cycles)
                        cpu.srl(Indirect(CPU::combine_bytes(cpu.register_h, cpu.register_l)));
                    }
                    // SWAP r  (2 M-cycles)
                    reg_num @ 0..=7 => cpu.swap(Register(reg_num)),
                    // SRL r  (2 M-cycles)
                    reg_num @ 8..=0xF => cpu.srl(Register(reg_num - 8)),
                    _ => (),
                },
                4..=7 => {
                    let bit_num = (arg & 0b00111000) >> 3;
                    let reg_num = arg & 0b00000111;
                    let hl = CPU::combine_bytes(cpu.register_h, cpu.register_l);
                    match arg_low_nibble {
                        // BIT n, r  (2 M-cycles)
                        6 | 0xE => cpu.bit(bit_num, Indirect(hl)),
                        // BIT n, (hl)  (3 M-cycles)
                        _ => cpu.bit(bit_num, Register(reg_num)),
                    }
                }
                8..=0xB => {
                    let bit_num = (arg & 0b00111000) >> 3;
                    let reg_num = arg & 0b00000111;
                    let hl = CPU::combine_bytes(cpu.register_h, cpu.register_l);
                    match arg_low_nibble {
                        // RES n, r  (2 M-cycles)
                        6 | 0xE => cpu.res(bit_num, Indirect(hl)),
                        // RES n, (hl)  (4 M-cycles)
                        _ => cpu.res(bit_num, Register(reg_num)),
                    }
                }
                0xC..=0xF => {
                    let bit_num = (arg & 0b00111000) >> 3;
                    let reg_num = arg & 0b00000111;
                    let hl = CPU::combine_bytes(cpu.register_h, cpu.register_l);
                    match arg_low_nibble {
                        // SET n, r  (2 M-cycles)
                        6 | 0xE => cpu.set(bit_num, Indirect(hl)),
                        // SET n, (hl)  (4 M-cycles)
                        _ => cpu.set(bit_num, Register(reg_num)),
                    }
                }
                _ => {}
            }
        }));

        // CCF  (1 M-cycles)
        cpu.instructions[0x3F as usize] = Some(Rc::new(move |cpu: &mut CPU| {
            let carry_flag = !(cpu.register_f | 0b11101111);
            cpu.register_f = cpu.register_f & 0b10000000 | carry_flag;
        }));

        // SCF  (1 M-cycles)
        cpu.instructions[0x37 as usize] = Some(Rc::new(move |cpu: &mut CPU| {
            cpu.register_f = cpu.register_f & 0b10000000 | 0b00010000;
        }));

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
        while self.program_counter as usize <= initial_pc + program.len() - 1 {
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

    fn get_operand(&mut self, op: Operand) -> Option<&mut u8> {
        match op {
            Register(r) => self.get_register(r),
            Indirect(a) => Some(self.get_memory_ref(a)),
        }
    }

    fn update_flags_add(&mut self, op1: u8, op2: u8) {
        self.register_f = self.register_f & 0b10111111;

        let mut sum = Wrapping(op1);
        sum += op2;
        let zero = sum.0 == 0;
        if zero {
            self.register_f = self.register_f | 0b10000000;
        } else {
            self.register_f = self.register_f & 0b01111111;
        }

        let overflow = op1 as u16 + op2 as u16 > 255;
        if overflow {
            self.register_f = self.register_f | 0b00010000;
        } else {
            self.register_f = self.register_f & 0b11101111;
        }

        let op1_low_nib = op1 & 0b00001111;
        let op2_low_nib = op2 & 0b00001111;
        let half_carry = op1_low_nib + op2_low_nib > 0xF;
        if half_carry {
            self.register_f = self.register_f | 0b00100000;
        } else {
            self.register_f = self.register_f & 0b11011111;
        }
    }

    fn update_flags_sub(&mut self, op1: u8, op2: u8) {
        self.register_f = self.register_f | 0b01000000;

        let mut sum = Wrapping(op1);
        sum += op2;
        let zero = sum.0 == 0;
        if zero {
            self.register_f = self.register_f | 0b10000000;
        } else {
            self.register_f = self.register_f & 0b01111111;
        }

        let underflow = CPU::negate(op2) > op1;
        if underflow {
            self.register_f = self.register_f | 0b00010000;
        } else {
            self.register_f = self.register_f & 0b11101111;
        }

        let op1_low_nib = op1 & 0b00001111;
        let op2_low_nib = CPU::negate(op2) & 0b00001111;
        let half_carry = op2_low_nib > op1_low_nib;
        if half_carry {
            self.register_f = self.register_f | 0b00100000;
        } else {
            self.register_f = self.register_f & 0b11011111;
        }
    }

    fn update_hc_flags_add_u16(&mut self, op1: u16, op2: u16) {
        let overflow = u16::MAX - op1 < op2;
        if overflow {
            self.register_f = self.register_f | 0b00010000;
        } else {
            self.register_f = self.register_f & 0b11101111;
        }

        let op1_low = op1 & 0x0FFF;
        let op2_low = op2 & 0x0FFF;
        let half_carry = op1_low + op2_low > 0x0FFF;
        if half_carry {
            self.register_f = self.register_f | 0b00100000;
        } else {
            self.register_f = self.register_f & 0b11011111;
        }
    }

    fn negate(num: u8) -> u8 {
        if num == 0 {
            0
        } else {
            !num + 1
        }
    }

    fn get_carry_bit(&self) -> u8 {
        return (self.register_f & 0b00010000) >> 4;
    }

    fn rlc(&mut self, op: Operand) {
        let source_option = self.get_operand(op);
        if let Some(source) = source_option {
            let bit_seven = (*source & 0b10000000) >> 7;
            *source = *source << 1 | bit_seven;
            let is_zero = if *source == 0 { 1 } else { 0 };
            self.register_f = (self.register_f & 0b00000000) | bit_seven << 4 | is_zero << 7;
        }
    }

    fn rl(&mut self, op: Operand, carry_bit: u8) {
        let source_option = self.get_operand(op);
        if let Some(source) = source_option {
            let bit_seven = (*source & 0b10000000) >> 7;
            *source = *source << 1 | carry_bit;
            let is_zero = if *source == 0 { 1 } else { 0 };
            self.register_f = (self.register_f & 0b00000000) | bit_seven << 4 | is_zero << 7;
        }
    }

    fn rrc(&mut self, op: Operand) {
        let source_option = self.get_operand(op);
        if let Some(source) = source_option {
            let bit_zero = *source & 0b00000001;
            *source = *source >> 1 | bit_zero << 7;
            let is_zero = if *source == 0 { 1 } else { 0 };
            self.register_f = (self.register_f & 0b00000000) | bit_zero << 4 | is_zero << 7;
        }
    }

    fn rr(&mut self, op: Operand, carry_bit: u8) {
        let source_option = self.get_operand(op);
        if let Some(source) = source_option {
            let bit_zero = *source & 0b00000001;
            *source = *source >> 1 | carry_bit << 7;
            let is_zero = if *source == 0 { 1 } else { 0 };
            self.register_f = (self.register_f & 0b00000000) | bit_zero << 4 | is_zero << 7;
        }
    }

    fn sla(&mut self, op: Operand) {
        let source_option = self.get_operand(op);
        if let Some(source) = source_option {
            let carry_bit = (*source & 0b10000000) >> 7;
            *source = *source << 1;
            let zero_bit = if *source == 0 { 1 } else { 0 };
            self.register_f = 0b00000000 | carry_bit << 4 | zero_bit << 7;
        }
    }

    fn sra(&mut self, op: Operand) {
        let source_option = self.get_operand(op);
        if let Some(source) = source_option {
            let bit_seven = *source & 0b10000000;
            let carry_bit = *source & 0b00000001;
            *source = (*source >> 1) | bit_seven;
            let zero_bit = if *source == 0 { 1 } else { 0 };
            self.register_f = 0b00000000 | carry_bit << 4 | zero_bit << 7;
        }
    }

    fn srl(&mut self, op: Operand) {
        let source_option = self.get_operand(op);
        if let Some(source) = source_option {
            let carry_bit = *source & 0b00000001;
            *source = *source >> 1;
            let zero_bit = if *source == 0 { 1 } else { 0 };
            self.register_f = 0b00000000 | carry_bit << 4 | zero_bit << 7;
        }
    }

    fn swap(&mut self, op: Operand) {
        let source_option = self.get_operand(op);
        if let Some(source) = source_option {
            let high_nibble = *source & 0b11110000;
            let low_nibble = *source & 0b00001111;
            *source = low_nibble << 4 | high_nibble >> 4;
            let zero_bit = if *source == 0 { 1 } else { 0 };
            self.register_f = 0b00000000 | zero_bit << 7;
        }
    }

    fn bit(&mut self, bit_num: u8, op: Operand) {
        let source_option = self.get_operand(op);
        if let Some(source) = source_option {
            let mask = 1 << bit_num;
            let test_bit = (*source & mask) >> bit_num;
            self.register_f = self.register_f & 0b00011111 | 0b00100000 | test_bit << 7;
        }
    }

    fn res(&mut self, bit_num: u8, op: Operand) {
        let source_option = self.get_operand(op);
        if let Some(source) = source_option {
            let mask = !(1 << bit_num);
            *source = *source & mask;
        }
    }

    fn set(&mut self, bit_num: u8, op: Operand) {
        let source_option = self.get_operand(op);
        if let Some(source) = source_option {
            let set_bit = 1 << bit_num;
            *source = *source | set_bit;
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

    fn get_memory_ref(&mut self, address: u16) -> &mut u8 {
        &mut self.memory[address as usize]
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

    #[test]
    fn add_basic_b() {
        let mut cpu = CPU::new();
        cpu.run_test(vec![0x80]);
        assert_eq!(cpu.register_a, 0x11);
    }

    #[test]
    fn add_basic_l() {
        let mut cpu = CPU::new();
        cpu.run_test(vec![0x85]);
        assert_eq!(cpu.register_a, 0x1E);
    }

    #[test]
    fn add_basic_immediate() {
        let mut cpu = CPU::new();
        cpu.run_test(vec![0xC6, 0x12]);
        assert_eq!(cpu.register_a, 0x23);
    }

    #[test]
    fn add_basic_hl() {
        let mut cpu = CPU::new();
        // ld (hl), $02
        // add a, (hl)
        cpu.run_test(vec![0x36, 0x02, 0x86]);
        assert_eq!(cpu.register_a, 0x13);
    }

    #[test]
    fn add_a_has_correct_value_after_overflow() {
        let mut cpu = CPU::new();
        cpu.run_test(vec![0xC6, 0xFF]);
        assert_eq!(cpu.register_a, 0x10);
    }

    #[test]
    fn add_zero_flag_is_one() {
        let mut cpu = CPU::new();
        // ld a, 0
        // add a, 0
        cpu.run_test(vec![0x3E, 0x00, 0xC6, 0x00]);
        let zero_bit = cpu.register_f & 0b10000000;
        assert_eq!(zero_bit, 128);
    }

    #[test]
    fn add_zero_flag_is_one_with_overflow() {
        let mut cpu = CPU::new();
        cpu.run_test(vec![0xC6, 0xEE]);
        let zero_bit = cpu.register_f & 0b00010000;
        assert_eq!(zero_bit, 0);
    }

    #[test]
    fn add_zero_flag_is_zero() {
        let mut cpu = CPU::new();
        cpu.run_test(vec![0xC6, 0x11]);
        let zero_bit = cpu.register_f & 0b10000000;
        assert_eq!(zero_bit, 0);
    }

    #[test]
    fn add_carry_flag_is_zero_no_overflow() {
        let mut cpu = CPU::new();
        cpu.run_test(vec![0xC6, 0x01]);
        let overflow_bit = cpu.register_f & 0b00010000;
        assert_eq!(overflow_bit, 0);
    }

    #[test]
    fn add_carry_flag_is_one_after_overflow() {
        let mut cpu = CPU::new();
        cpu.run_test(vec![0xC6, 0xFF]);
        let overflow_bit = cpu.register_f & 0b00010000;
        assert_eq!(overflow_bit, 16);
    }

    #[test]
    fn add_half_carry_flag_is_zero() {
        let mut cpu = CPU::new();
        cpu.run_test(vec![0xC6, 0x01]);
        let half_carry_bit = cpu.register_f & 0b00100000;
        assert_eq!(half_carry_bit, 0);
    }

    #[test]
    fn add_half_carry_flag_is_one() {
        let mut cpu = CPU::new();
        // ld a, $08
        // add a, $08
        cpu.run_test(vec![0x3E, 0x08, 0xC6, 0x08]);
        let half_carry_bit = cpu.register_f & 0b00100000;
        assert_eq!(half_carry_bit, 32);
    }

    #[test]
    fn add_half_carry_flag_is_one_carried_from_bit_1() {
        let mut cpu = CPU::new();
        // ld a, $0A
        // add a, $07
        cpu.run_test(vec![0x3E, 0x0A, 0xC6, 0x07]);
        let half_carry_bit = cpu.register_f & 0b00100000;
        assert_eq!(half_carry_bit, 32);
    }

    #[test]
    fn add_subtraction_flag_is_zero() {
        let mut cpu = CPU::new();
        cpu.run_test(vec![0xC6, 0x01]);
        let subtraction_bit = cpu.register_f & 0b01000000;
        assert_eq!(subtraction_bit, 0);
    }

    #[test]
    fn add_multiple_times_and_flags_reflect_most_recent() {
        let mut cpu = CPU::new();
        // Add a, $FF (overflow flag will be 1 at this point: same as prior test)
        // Add a, $01 (overflow flag should be 0 now)
        cpu.run_test(vec![0xC6, 0xFF, 0xC6, 0x01]);
        let overflow_bit = cpu.register_f & 0b00010000;
        assert_eq!(overflow_bit, 0);
    }

    #[test]
    fn adc_e_when_carry_flag_is_zero() {
        let mut cpu = CPU::new();
        cpu.run_test(vec![0x8B]);
        assert_eq!(cpu.register_a, 0x67);
    }

    #[test]
    fn adc_e_when_carry_flag_is_one() {
        let mut cpu = CPU::new();
        cpu.register_f = cpu.register_f | 0b00010000;
        cpu.run_test(vec![0x8B]);
        assert_eq!(cpu.register_a, 0x68);
    }

    #[test]
    fn adc_n() {
        let mut cpu = CPU::new();
        cpu.register_f = cpu.register_f | 0b00010000;
        cpu.run_test(vec![0xCE, 0x02]);
        assert_eq!(cpu.register_a, 0x14);
    }

    #[test]
    fn adc_hl() {
        let mut cpu = CPU::new();
        // ld (hl), $02
        // add a, (hl)
        cpu.register_f = cpu.register_f | 0b00010000;
        cpu.run_test(vec![0x36, 0x02, 0x8E]);
        assert_eq!(cpu.register_a, 0x14);
    }

    #[test]
    fn sub_c_basic() {
        let mut cpu = CPU::new();
        cpu.run_test(vec![0x91]);
        assert_eq!(cpu.register_a, 0x11);
    }

    #[test]
    fn sub_b_basic() {
        let mut cpu = CPU::new();
        cpu.register_b = 0x10;
        cpu.run_test(vec![0x90]);
        assert_eq!(cpu.register_a, 0x01);
    }

    #[test]
    fn sub_basic_immediate() {
        let mut cpu = CPU::new();
        cpu.run_test(vec![0xD6, 0x08]);
        assert_eq!(cpu.register_a, 0x09);
    }

    #[test]
    fn sub_basic_hl() {
        let mut cpu = CPU::new();
        // ld (hl), $02
        // sub a, (hl)
        cpu.run_test(vec![0x36, 0x02, 0x96]);
        assert_eq!(cpu.register_a, 0x0F);
    }

    #[test]
    fn sub_zero_flag_is_one() {
        let mut cpu = CPU::new();
        cpu.run_test(vec![0xD6, 0x11]);
        let zero_bit = cpu.register_f & 0b10000000;
        assert_eq!(zero_bit, 128);
    }

    #[test]
    fn sub_zero_flag_is_one_with_underflow() {
        let mut cpu = CPU::new();
        cpu.run_test(vec![0xD6, 0xEE]);
        let zero_bit = cpu.register_f & 0b00010000;
        assert_eq!(zero_bit, 16);
    }

    #[test]
    fn sub_zero_flag_is_zero() {
        let mut cpu = CPU::new();
        cpu.run_test(vec![0xD6, 0x01]);
        let zero_bit = cpu.register_f & 0b10000000;
        assert_eq!(zero_bit, 0);
    }

    #[test]
    fn sub_a_has_correct_value_after_underflow() {
        let mut cpu = CPU::new();
        cpu.run_test(vec![0xD6, 0x12]);
        assert_eq!(cpu.register_a, 0xFF);
    }

    #[test]
    fn sub_carry_flag_is_one_after_underflow() {
        let mut cpu = CPU::new();
        cpu.run_test(vec![0xD6, 0x12]);
        let carry_bit = cpu.register_f & 0b00010000;
        assert_eq!(carry_bit, 16);
    }

    #[test]
    fn sub_carry_flag_is_zero_without_underflow() {
        let mut cpu = CPU::new();
        cpu.run_test(vec![0xD6, 0x10]);
        let carry_bit = cpu.register_f & 0b00010000;
        assert_eq!(carry_bit, 0);
    }

    #[test]
    fn sub_half_carry_flag_is_zero() {
        let mut cpu = CPU::new();
        cpu.run_test(vec![0xD6, 0x01]);
        let half_carry_bit = cpu.register_f & 0b00100000;
        assert_eq!(half_carry_bit, 0);
    }

    #[test]
    fn sub_half_carry_flag_is_one() {
        let mut cpu = CPU::new();
        cpu.run_test(vec![0xD6, 0x08]);
        let half_carry_bit = cpu.register_f & 0b00100000;
        assert_eq!(half_carry_bit, 32);
    }

    #[test]
    fn sub_half_carry_flag_is_one_borrow_across_multiple_bits() {
        let mut cpu = CPU::new();
        cpu.run_test(vec![0xD6, 0x02]);
        let half_carry_bit = cpu.register_f & 0b00100000;
        assert_eq!(half_carry_bit, 32);
    }

    #[test]
    fn sub_subtraction_flag_is_one() {
        let mut cpu = CPU::new();
        cpu.run_test(vec![0xD6, 0x01]);
        let subtraction_bit = cpu.register_f & 0b01000000;
        assert_eq!(subtraction_bit, 64);
    }

    #[test]
    fn sbc_b_when_carry_flag_is_zero() {
        let mut cpu = CPU::new();
        cpu.run_test(vec![0x98]);
        assert_eq!(cpu.register_a, 0x11);
    }

    #[test]
    fn sbc_b_when_carry_flag_is_one() {
        let mut cpu = CPU::new();
        cpu.register_f = cpu.register_f | 0b00010000;
        cpu.run_test(vec![0x98]);
        assert_eq!(cpu.register_a, 0x10);
    }

    #[test]
    fn sbc_n() {
        let mut cpu = CPU::new();
        cpu.register_f = cpu.register_f | 0b00010000;
        cpu.run_test(vec![0xDE, 0x10]);
        assert_eq!(cpu.register_a, 0x00);
    }

    #[test]
    fn sbc_hl() {
        let mut cpu = CPU::new();
        // ld (hl), $02
        // sbc a, (hl)
        cpu.register_f = cpu.register_f | 0b00010000;
        cpu.run_test(vec![0x36, 0x02, 0x9E]);
        assert_eq!(cpu.register_a, 0x11 - 3);
    }

    #[test]
    fn and_b() {
        let mut cpu = CPU::new();
        cpu.run_test(vec![0xA0]);
        assert_eq!(cpu.register_a, 0)
    }

    #[test]
    fn and_b_flags_are_correct() {
        let mut cpu = CPU::new();
        cpu.run_test(vec![0xA0]);
        assert_eq!(cpu.register_f, 0b10100000);
    }

    #[test]
    fn and_a_flags_are_correct() {
        let mut cpu = CPU::new();
        cpu.run_test(vec![0xA7]);
        assert_eq!(cpu.register_f, 0b00100000);
    }

    #[test]
    fn and_immediate() {
        let mut cpu = CPU::new();
        cpu.run_test(vec![0xE6, 0b11110000]);
        assert_eq!(cpu.register_a, 0b00010000);
    }

    #[test]
    fn and_hl() {
        let mut cpu = CPU::new();
        cpu.run_test(vec![0xE6]);
        assert_eq!(cpu.register_a, 0);
    }

    #[test]
    fn xor_b() {
        let mut cpu = CPU::new();
        cpu.run_test(vec![0xA8]);
        assert_eq!(cpu.register_a, 0b00010001)
    }

    #[test]
    fn xor_b_flags_are_correct() {
        let mut cpu = CPU::new();
        cpu.run_test(vec![0xA8]);
        assert_eq!(cpu.register_f, 0b00000000);
    }

    #[test]
    fn xor_a_flags_are_correct() {
        let mut cpu = CPU::new();
        cpu.run_test(vec![0xAF]);
        assert_eq!(cpu.register_f, 0b10000000);
    }

    #[test]
    fn xor_immediate() {
        let mut cpu = CPU::new();
        cpu.run_test(vec![0xEE, 0b11111111]);
        assert_eq!(cpu.register_a, 0b11101110);
    }

    #[test]
    fn xor_hl() {
        let mut cpu = CPU::new();
        cpu.run_test(vec![0xAE]);
        assert_eq!(cpu.register_a, 0b00010001);
    }

    #[test]
    fn or_b() {
        let mut cpu = CPU::new();
        cpu.run_test(vec![0xB0]);
        assert_eq!(cpu.register_a, 0b00010001)
    }

    #[test]
    fn or_b_flags_are_correct() {
        let mut cpu = CPU::new();
        cpu.run_test(vec![0xB0]);
        assert_eq!(cpu.register_f, 0b00000000);
    }

    #[test]
    fn or_immediate() {
        let mut cpu = CPU::new();
        cpu.run_test(vec![0xF6, 0b11101110]);
        assert_eq!(cpu.register_a, 0b11111111);
    }

    #[test]
    fn or_hl() {
        let mut cpu = CPU::new();
        cpu.run_test(vec![0xB6]);
        assert_eq!(cpu.register_a, 0b00010001);
    }

    #[test]
    fn cp_b() {
        let mut cpu = CPU::new();
        cpu.register_b = 0x10;
        cpu.run_test(vec![0xB8]);
        assert_eq!(cpu.register_f, 0b01000000);
    }

    #[test]
    fn cp_immediate() {
        let mut cpu = CPU::new();
        cpu.run_test(vec![0xFE, 0x08]);
        let half_carry_bit = cpu.register_f & 0b00100000;
        assert_eq!(cpu.register_f, 0b01100000);
    }

    #[test]
    fn cp_hl() {
        let mut cpu = CPU::new();
        // ld (hl), $02
        // cp a, (hl)
        cpu.run_test(vec![0x36, 0x02, 0xBE]);
        assert_eq!(cpu.register_f, 0b01100000);
    }

    #[test]
    fn inc_b_basic() {
        let mut cpu = CPU::new();
        cpu.run_test(vec![0x04]);
        assert_eq!(cpu.register_b, 0x01);
    }

    #[test]
    fn inc_b_overflow() {
        let mut cpu = CPU::new();
        // ld b 0xFF
        // inc b
        cpu.run_test(vec![0x06, 0xFF, 0x04]);
        assert_eq!(cpu.register_b, 0x00);
    }

    #[test]
    fn inc_hl() {
        let mut cpu = CPU::new();
        // ld (hl) 0x02
        // inc (hl)
        cpu.run_test(vec![0x36, 0x02, 0x34]);
        assert_eq!(
            cpu.read(CPU::combine_bytes(cpu.register_h, cpu.register_l)),
            0x03
        );
    }

    #[test]
    fn inc_doesnt_change_carry_flag() {
        let mut cpu = CPU::new();
        // ld b 0xFF
        // inc b
        let initial_carry_bit = 0b00010000 & cpu.register_f;
        cpu.run_test(vec![0x06, 0xFF, 0x04]);
        let carry_bit = 0b00010000 & cpu.register_f;
        assert_eq!(carry_bit, initial_carry_bit);
    }

    #[test]
    fn dec_b_basic() {
        let mut cpu = CPU::new();
        // ld b 0xFF
        // dec b
        cpu.run_test(vec![0x06, 0xFF, 0x05]);
        assert_eq!(cpu.register_b, 0xFE);
    }

    #[test]
    fn dec_b_underflow() {
        let mut cpu = CPU::new();
        cpu.run_test(vec![0x05]);
        assert_eq!(cpu.register_b, 0xFF);
    }

    #[test]
    fn dec_hl() {
        let mut cpu = CPU::new();
        // ld (hl) 0x02
        // dec (hl)
        cpu.run_test(vec![0x36, 0x02, 0x35]);
        assert_eq!(
            cpu.read(CPU::combine_bytes(cpu.register_h, cpu.register_l)),
            0x01
        );
    }

    #[test]
    fn dec_doesnt_change_carry_flag() {
        let mut cpu = CPU::new();
        let initial_carry_bit = 0b00010000 & cpu.register_f;
        cpu.run_test(vec![0x05]);
        let carry_bit = 0b00010000 & cpu.register_f;
        assert_eq!(carry_bit, initial_carry_bit);
    }

    #[test]
    fn daa_both_digits_within_limit() {
        let mut cpu = CPU::new();
        cpu.register_a = 0x99;
        cpu.run_test(vec![0x27]);
        assert_eq!(cpu.register_a, 0x99);
        assert_eq!(cpu.register_f, 0b00000000);
    }

    #[test]
    fn daa_lsb_outside_limit() {
        let mut cpu = CPU::new();
        cpu.register_a = 0x0A;
        cpu.run_test(vec![0x27]);
        assert_eq!(cpu.register_a, 0x10);
        assert_eq!(cpu.register_f, 0b00000000);
    }

    #[test]
    fn daa_msb_outside_limit() {
        let mut cpu = CPU::new();
        cpu.register_a = 0xA0;
        cpu.run_test(vec![0x27]);
        assert_eq!(cpu.register_a, 0x00);
        assert_eq!(cpu.register_f, 0b10010000);
    }

    #[test]
    fn daa_overflow_to_zero() {
        let mut cpu = CPU::new();
        cpu.register_a = 0x9A;
        cpu.run_test(vec![0x27]);
        assert_eq!(cpu.register_a, 0x00);
        assert_eq!(cpu.register_f, 0b10010000);
    }

    #[test]
    fn cpl_basic() {
        let mut cpu = CPU::new();
        cpu.run_test(vec![0x2F]);
        assert_eq!(cpu.register_a, 0b11101110);
    }

    #[test]
    fn cpl_flags_are_correct() {
        let mut cpu = CPU::new();
        cpu.run_test(vec![0x2F]);
        assert_eq!(cpu.register_f, 0b11100000);
    }

    #[test]
    fn add_hl_bc() {
        let mut cpu = CPU::new();
        cpu.run_test(vec![0x09]);
        assert_eq!((cpu.register_h, cpu.register_l), (0x00, 0x0D));
    }

    #[test]
    fn add_hl_sp() {
        let mut cpu = CPU::new();
        cpu.run_test(vec![0x39]);
        assert_eq!((cpu.register_h, cpu.register_l), (0x00, 0x0B));
    }

    #[test]
    fn add_hl_sp_flags_are_correct() {
        let mut cpu = CPU::new();
        cpu.run_test(vec![0x39]);
        assert_eq!(cpu.register_f, 0b10110000);
    }

    #[test]
    fn inc_bc() {
        let mut cpu = CPU::new();
        cpu.run_test(vec![0x03]);
        assert_eq!((cpu.register_b, cpu.register_c), (0x00, 0x01));
    }

    #[test]
    fn inc_sp() {
        let mut cpu = CPU::new();
        cpu.run_test(vec![0x33]);
        assert_eq!(cpu.stack_pointer, 0xFFFF);
    }

    #[test]
    fn inc_doesnt_change_flags() {
        let mut cpu = CPU::new();
        let initial_flags = cpu.register_f;
        cpu.run_test(vec![0x33]);
        assert_eq!(cpu.register_f, initial_flags);
    }

    #[test]
    fn dec_bc() {
        let mut cpu = CPU::new();
        cpu.run_test(vec![0x0B]);
        assert_eq!((cpu.register_b, cpu.register_c), (0xFF, 0xFF));
    }

    #[test]
    fn dec_sp() {
        let mut cpu = CPU::new();
        cpu.run_test(vec![0x3B]);
        assert_eq!(cpu.stack_pointer, 0xFFFD);
    }

    #[test]
    fn dec_doesnt_change_flags() {
        let mut cpu = CPU::new();
        let initial_flags = cpu.register_f;
        cpu.run_test(vec![0x0B]);
        assert_eq!(cpu.register_f, initial_flags);
    }

    #[test]
    fn add_sp_dd() {
        let mut cpu = CPU::new();
        cpu.run_test(vec![0xE8, 0xF0]);
        assert_eq!(cpu.stack_pointer, 0x00EE);
    }

    #[test]
    fn ld_hl_sp_dd() {
        let mut cpu = CPU::new();
        cpu.run_test(vec![0xF8, 0xF0]);
        assert_eq!(CPU::combine_bytes(cpu.register_h, cpu.register_l), 0x00EE);
        assert_eq!(cpu.stack_pointer, 0xFFFE);
    }

    #[test]
    fn rlca_bit_zero_is_zero() {
        let mut cpu = CPU::new();
        cpu.run_test(vec![0x07]);
        assert_eq!(cpu.register_a, 0b00100010);
    }

    #[test]
    fn rlca_bit_zero_is_one() {
        let mut cpu = CPU::new();
        cpu.register_a = 0b10000000;
        cpu.run_test(vec![0x07]);
        assert_eq!(cpu.register_a, 0b00000001);
    }

    #[test]
    fn rlca_carry_flag_is_zero() {
        let mut cpu = CPU::new();
        cpu.run_test(vec![0x07]);
        assert_eq!(cpu.register_f, 0b00000000);
    }

    #[test]
    fn rlca_carry_flag_is_one() {
        let mut cpu = CPU::new();
        cpu.register_a = 0b10000000;
        cpu.run_test(vec![0x07]);
        assert_eq!(cpu.register_f, 0b00010000);
    }

    #[test]
    fn rla_carry_bit_becomes_bit_seven() {
        let mut cpu = CPU::new();
        cpu.register_f = 0b10010000;
        cpu.register_a = 0b01111111;
        cpu.run_test(vec![0x17]);
        assert_eq!(cpu.register_f, 0b00000000);
    }

    #[test]
    fn rla_bit_zero_becomes_carry_bit() {
        let mut cpu = CPU::new();
        cpu.register_f = 0b00010000;
        cpu.register_a = 0b00000000;
        cpu.run_test(vec![0x17]);
        assert_eq!(cpu.register_a, 0b000000001);
    }

    #[test]
    fn rrca() {
        let mut cpu = CPU::new();
        cpu.register_a = 0b10000001;
        cpu.run_test(vec![0x0F]);
        assert_eq!(cpu.register_a, 0b11000000);
    }

    #[test]
    fn rrca_flags() {
        let mut cpu = CPU::new();
        cpu.register_a = 0b10000001;
        cpu.run_test(vec![0x0F]);
        assert_eq!(cpu.register_f, 0b00010000);
    }

    #[test]
    fn rra() {
        let mut cpu = CPU::new();
        cpu.register_a = 0b10000000;
        cpu.register_f = 0b00010000;
        cpu.run_test(vec![0x1F]);
        assert_eq!(cpu.register_a, 0b11000000);
    }

    #[test]
    fn rra_flags() {
        let mut cpu = CPU::new();
        cpu.register_a = 0b10000000;
        cpu.register_f = 0b00010000;
        cpu.run_test(vec![0x1F]);
        assert_eq!(cpu.register_f, 0b00000000);
    }

    #[test]
    fn rlc_hl() {
        // Mainly just to test the 0xCB instructions are mapped correctly
        let mut cpu = CPU::new();
        let hl = CPU::combine_bytes(cpu.register_h, cpu.register_l);
        cpu.write(hl, 0b10000000);
        cpu.run_test(vec![0xCB, 0x06]);
        assert_eq!(cpu.read(hl), 0b00000001);
    }

    #[test]
    fn sla_b() {
        let mut cpu = CPU::new();
        cpu.register_b = 0b01010011;
        cpu.run_test(vec![0xCB, 0x20]);
        assert_eq!(cpu.register_b, 0b10100110);
    }

    #[test]
    fn sla_b_flags() {
        let mut cpu = CPU::new();
        cpu.register_b = 0b11010011;
        cpu.run_test(vec![0xCB, 0x20]);
        assert_eq!(cpu.register_f, 0b00010000);
    }

    #[test]
    fn sla_hl() {
        let mut cpu = CPU::new();
        let hl = CPU::combine_bytes(cpu.register_h, cpu.register_l);
        cpu.write(hl, 0b01011111);
        cpu.run_test(vec![0xCB, 0x26]);
        assert_eq!(cpu.read(hl), 0b10111110);
    }

    #[test]
    fn sra_b() {
        let mut cpu = CPU::new();
        cpu.register_b = 0b11010000;
        cpu.run_test(vec![0xCB, 0x28]);
        assert_eq!(cpu.register_b, 0b11101000);
    }

    #[test]
    fn sra_b_flags() {
        let mut cpu = CPU::new();
        cpu.register_b = 0b01010011;
        cpu.run_test(vec![0xCB, 0x28]);
        assert_eq!(cpu.register_f, 0b00010000);
    }

    #[test]
    fn sra_hl() {
        let mut cpu = CPU::new();
        let hl = CPU::combine_bytes(cpu.register_h, cpu.register_l);
        cpu.write(hl, 0b01000001);
        cpu.run_test(vec![0xCB, 0x2E]);
        assert_eq!(cpu.read(hl), 0b00100000);
    }

    #[test]
    fn srl_b() {
        let mut cpu = CPU::new();
        cpu.register_b = 0b11010000;
        cpu.run_test(vec![0xCB, 0x38]);
        assert_eq!(cpu.register_b, 0b01101000);
    }

    #[test]
    fn srl_hl() {
        let mut cpu = CPU::new();
        let hl = CPU::combine_bytes(cpu.register_h, cpu.register_l);
        cpu.write(hl, 0b01000001);
        cpu.run_test(vec![0xCB, 0x3E]);
        assert_eq!(cpu.read(hl), 0b00100000);
    }

    #[test]
    fn swap_b() {
        let mut cpu = CPU::new();
        cpu.register_b = 0b11010000;
        cpu.run_test(vec![0xCB, 0x30]);
        assert_eq!(cpu.register_b, 0b00001101);
    }

    #[test]
    fn swap_b_flags() {
        let mut cpu = CPU::new();
        cpu.register_b = 0b11010000;
        cpu.run_test(vec![0xCB, 0x30]);
        assert_eq!(cpu.register_f, 0b00000000);
    }

    #[test]
    fn swap_hl() {
        let mut cpu = CPU::new();
        let hl = CPU::combine_bytes(cpu.register_h, cpu.register_l);
        cpu.write(hl, 0b01000001);
        cpu.run_test(vec![0xCB, 0x36]);
        assert_eq!(cpu.read(hl), 0b00010100);
    }

    #[test]
    fn bit_0_b_is_zero() {
        let mut cpu = CPU::new();
        cpu.run_test(vec![0xCB, 0x40]);
        assert_eq!(cpu.register_f, 0b00100000);
    }

    #[test]
    fn bit_7_b_is_one() {
        let mut cpu = CPU::new();
        cpu.register_b = 0b10000000;
        cpu.run_test(vec![0xCB, 0x78]);
        assert_eq!(cpu.register_f, 0b10100000);
    }

    #[test]
    fn bit_5_hl_is_one() {
        let mut cpu = CPU::new();
        let hl = CPU::combine_bytes(cpu.register_h, cpu.register_l);
        cpu.write(hl, 0b00100000);
        cpu.run_test(vec![0xCB, 0x6E]);
        assert_eq!(cpu.register_f, 0b10100000);
    }

    #[test]
    fn bit_4_hl_is_zero() {
        let mut cpu = CPU::new();
        let hl = CPU::combine_bytes(cpu.register_h, cpu.register_l);
        cpu.write(hl, 0b00100000);
        cpu.run_test(vec![0xCB, 0x66]);
        assert_eq!(cpu.register_f, 0b00100000);
    }

    #[test]
    fn res_0_a() {
        let mut cpu = CPU::new();
        cpu.run_test(vec![0xCB, 0x87]);
        assert_eq!(cpu.register_a, 0b00010000);
    }

    #[test]
    fn res_4_a() {
        let mut cpu = CPU::new();
        cpu.run_test(vec![0xCB, 0xA7]);
        assert_eq!(cpu.register_a, 0b00000001);
    }

    #[test]
    fn res_7_d() {
        let mut cpu = CPU::new();
        cpu.run_test(vec![0xCB, 0xBA]);
        assert_eq!(cpu.register_d, 0b01111111);
    }

    #[test]
    fn res_0_b_doesnt_change_bit_if_already_zero() {
        let mut cpu = CPU::new();
        cpu.run_test(vec![0xCB, 0x80]);
        assert_eq!(cpu.register_b, 0b00000000);
    }

    #[test]
    fn set_0_c() {
        let mut cpu = CPU::new();
        cpu.run_test(vec![0xCB, 0xC1]);
        assert_eq!(cpu.register_c, 0b00000001);
    }

    #[test]
    fn set_2_c() {
        let mut cpu = CPU::new();
        cpu.run_test(vec![0xCB, 0xD1]);
        assert_eq!(cpu.register_c, 0b00000100);
    }

    #[test]
    fn set_7_c() {
        let mut cpu = CPU::new();
        cpu.run_test(vec![0xCB, 0xF9]);
        assert_eq!(cpu.register_c, 0b10000000);
    }

    #[test]
    fn set_7_d_doesnt_change_bit_if_already_one() {
        let mut cpu = CPU::new();
        cpu.run_test(vec![0xCB, 0xFA]);
        assert_eq!(cpu.register_d, 0b11111111);
    }

    #[test]
    fn ccf_cy_was_one() {
        let mut cpu = CPU::new();
        cpu.register_f = 0b00010000;
        cpu.run_test(vec![0x3F]);
        assert_eq!(cpu.register_f, 0b00000000);
    }

    #[test]
    fn ccf_cy_was_zero() {
        let mut cpu = CPU::new();
        cpu.run_test(vec![0x3F]);
        assert_eq!(cpu.register_f, 0b10010000);
    }

    #[test]
    fn scf_cy_was_one() {
        let mut cpu = CPU::new();
        cpu.register_f = 0b00010000;
        cpu.run_test(vec![0x37]);
        assert_eq!(cpu.register_f, 0b00010000);
    }

    #[test]
    fn scf_cy_was_zero() {
        let mut cpu = CPU::new();
        cpu.run_test(vec![0x37]);
        assert_eq!(cpu.register_f, 0b10010000);
    }
}
