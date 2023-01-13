use std::cell::RefCell;
use std::fs;
use std::rc::Rc;

use sdl2::pixels::PixelFormatEnum;

use crate::cpu::CPU;
use crate::mbc::mbc1::MBC1;
use crate::mbc::MBC;
use crate::mem_manager::MemManager;
use crate::memory::Memory;
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
        let program = fs::read(rom_path)?;
        // Preload cartridge header to to get data for setup
        let header_range = 0..0x014F;
        for i in header_range {
            self.memory.borrow_mut().write(i as u16, program[i]);
        }

        // MBC setup
        let rom_banks = self.get_number_of_rom_banks();
        let ram_banks = self.get_number_of_ram_banks();
        let mut mbc = self.get_mbc(rom_banks, ram_banks);

        // Load rom into memory
        let rom_bank_size: usize = 0x4000;
        if let Some(ref mut mbc) = mbc {
            let rom_select_address = 0x2000;
            for i in 0..rom_banks {
                mbc.write(rom_select_address, i);
                // Figure out whether the data should be written to first or second area of rom
                let bank_offset = if i == 0 || i == 0x20 || i == 0x40 || i == 0x60 {
                    0
                } else {
                    0x4000
                };
                for j in 0..rom_bank_size {
                    mbc.init_write(
                        bank_offset + j as u16,
                        program[rom_bank_size * i as usize + j],
                    )
                }
            }

            // Set rom select register back to initial value of zero
            mbc.write(rom_select_address, 0);
        } else {
            for i in 0..rom_bank_size * 2 {
                self.memory
                    .borrow_mut()
                    .write(i as u16, program[i as usize]);
            }
        }
        self.memory.borrow_mut().set_mbc(mbc);
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
        let mut poll_timer = 0;
        loop {
            poll_timer += 1;
            if poll_timer == 1000 {
                poll_timer -= 1000;
                for e in event_pump.poll_iter() {}
            }
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

    fn get_number_of_rom_banks(&self) -> u8 {
        2 << self.memory.borrow().read(0x0148)
    }

    fn get_number_of_ram_banks(&self) -> u8 {
        let header_value = self.memory.borrow().read(0x0149);
        match header_value {
            0 => 0,
            2 => 1,
            3 => 4,
            4 => 16,
            5 => 8,
            _ => 0,
        }
    }

    fn get_mbc(&self, rom_banks: u8, ram_banks: u8) -> Option<Box<dyn MBC>> {
        let header_value = self.memory.borrow().read(0x0147);
        match header_value {
            0 => None,
            1..=3 => Some(Box::new(MBC1::new(rom_banks, ram_banks))),
            _ => None,
        }
    }
}
