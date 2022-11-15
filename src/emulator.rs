use std::cell::RefCell;
use std::fs;
use std::rc::Rc;

use crate::cpu::CPU;
use crate::memory::{MemManager, Memory};
use crate::ppu::PPU;
use crate::timer::Timer;

const DOTS_PER_FRAME: u32 = 70224;

pub struct Emulator {
    memory: Rc<RefCell<MemManager>>,
    cpu: CPU,
    ppu: PPU,
    timer: Timer,
}

impl Emulator {
    pub fn new() -> Self {
        let mem = Rc::new(RefCell::new(MemManager::new()));
        Emulator {
            memory: mem.clone(),
            cpu: CPU::new(mem.clone()),
            ppu: PPU::new(mem.clone()),
            timer: Timer::new(mem.clone()),
        }
    }

    pub fn load_rom(&mut self, rom_path: &str) -> std::io::Result<()> {
        const ROM_LIMIT: u16 = 0x8000;
        let program = fs::read(rom_path)?;
        for i in 0..program.len() {
            if i >= ROM_LIMIT as usize {
                break;
            }
            self.memory.borrow_mut().write(i as u16, program[i]);
        }
        Ok(())
    }

    pub fn run(&mut self) {
        let mut dots = 0;
        loop {
            if dots >= DOTS_PER_FRAME {
                dots -= DOTS_PER_FRAME;
                // sleep until time for frame to be displayed
                // self.render(ppu.get_frame());
            }
            let curr_clocks = self.cpu.execute();
            self.timer.update(curr_clocks);
            self.ppu.update(curr_clocks);
            dots += curr_clocks;
        }
    }

    pub fn load_and_run(&mut self, rom_path: &str) {
        let status = self.load_rom(rom_path);
        if let Ok(_) = status {
            self.run();
        }
    }
}
