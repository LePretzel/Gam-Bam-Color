use std::{cell::RefCell, rc::Rc};

use crate::memory::MemManager;

pub struct PPU {
    mode: Option<Box<dyn Mode>>,
    memory: Rc<RefCell<MemManager>>,
    cycles: u32,
}

impl PPU {
    pub fn new(memory: Rc<RefCell<MemManager>>) -> Self {
        PPU {
            mode: Some(Box::new(Scan {
                memory: memory.clone(),
            })),
            memory: memory.clone(),
            cycles: 0,
        }
    }

    pub fn update(&mut self, cycles: u32) {
        if let Some(m) = &self.mode {
            m.update(cycles);
        }
    }

    fn transition(&mut self) {
        if let Some(m) = self.mode.take() {
            self.mode = Some(m.transition());
        }
    }
}

trait Mode {
    fn update(&self, cycles: u32);
    fn transition(self: Box<Self>) -> Box<dyn Mode>;
}

struct HBlank {
    memory: Rc<RefCell<MemManager>>,
}
impl Mode for HBlank {
    fn update(&self, cycles: u32) {
        // hblank
    }

    fn transition(self: Box<Self>) -> Box<dyn Mode> {
        // if at last line
        //ppu.current_mode = Box::new(Mode1);

        // else
        //ppu.current_mode = Box::new(Mode2);
        self
    }
}

struct VBlank {
    memory: Rc<RefCell<MemManager>>,
}
impl Mode for VBlank {
    fn update(&self, cycles: u32) {
        // vblank
    }

    fn transition(self: Box<Self>) -> Box<dyn Mode> {
        self
    }
}

struct Scan {
    memory: Rc<RefCell<MemManager>>,
}
impl Mode for Scan {
    fn update(&self, cycles: u32) {
        // oam scan
    }

    fn transition(self: Box<Self>) -> Box<dyn Mode> {
        self
    }
}

struct Draw {
    memory: Rc<RefCell<MemManager>>,
}
impl Mode for Draw {
    fn update(&self, cycles: u32) {
        // pixel drawing
    }

    fn transition(self: Box<Self>) -> Box<dyn Mode> {
        self
    }
}
