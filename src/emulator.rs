use std::cell::RefCell;
use std::fs;
use std::rc::Rc;

use sdl2::pixels::PixelFormatEnum;

use crate::cpu::CPU;
use crate::memory::{MemManager, Memory};
use crate::ppu::PPU;
use crate::timer::Timer;

const DOTS_PER_FRAME: u32 = 70224;
const SCREEN_WIDTH: u32 = 160;
const SCREEN_HEIGHT: u32 = 144;
const HORIZONTAL_SCALE: u32 = 5;
const VERTICAL_SCALE: u32 = 5;

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
        let sdl_context = sdl2::init().unwrap();
        let video_subsystem = sdl_context.video().unwrap();
        let window = video_subsystem
            .window(
                "Gam Bam Color",
                SCREEN_WIDTH * HORIZONTAL_SCALE,
                SCREEN_HEIGHT * VERTICAL_SCALE,
            )
            .position_centered()
            .build()
            .unwrap();

        let mut event_pump = sdl_context.event_pump().unwrap();
        let mut canvas = window.into_canvas().build().unwrap();

        let creator = canvas.texture_creator();
        let mut texture = creator
            .create_texture_target(PixelFormatEnum::RGB555, SCREEN_WIDTH, SCREEN_HEIGHT)
            .unwrap();

        let mut dots = 0;
        loop {
            for e in event_pump.poll_iter() {}
            if dots >= DOTS_PER_FRAME {
                dots -= DOTS_PER_FRAME;
                // Todo: sleep until time for frame to be displayed

                let frame = self.ppu.get_frame();
                texture
                    .update(None, &frame, (SCREEN_WIDTH * 2) as usize)
                    .unwrap();
                canvas.copy(&texture, None, None).unwrap();
                canvas.present();
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
