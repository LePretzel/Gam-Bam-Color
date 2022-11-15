use std::{cell::RefCell, rc::Rc};

use crate::memory::{MemManager, Memory};

const IF_ADDRESS: u16 = 0xFF0F;
const LCDC_ADDRESS: u16 = 0xFF40;
const STAT_ADDRESS: u16 = 0xFF41;
const SCY_ADDRESS: u16 = 0xFF42;
const SCX_ADDRESS: u16 = 0xFF43;
const LY_ADDRESS: u16 = 0xFF44;
const LYC_ADDRESS: u16 = 0xFF45;

pub struct PPU {
    mode: Rc<dyn Mode>,
    memory: Rc<RefCell<MemManager>>,
    // current_frame,
    // completed_frame,
    extra_dots: u32,
    mode_dots_passed: u32,
}

impl PPU {
    pub fn new(memory: Rc<RefCell<MemManager>>) -> Self {
        let initial_mode = Rc::new(Scan);
        let mut ppu = PPU {
            mode: initial_mode.clone(),
            memory: memory.clone(),
            extra_dots: 0,
            mode_dots_passed: 0,
        };
        ppu.set_mode(initial_mode.clone());
        ppu
    }

    pub fn update(&mut self, dots: u32) {
        let m = self.mode.clone();
        m.update(self, dots);
    }

    fn current_scanline(&self) -> u8 {
        self.memory.borrow().read(LY_ADDRESS)
    }

    // fn update_scanline **should check lyc=ly here as well

    fn set_mode(&mut self, mode: Rc<dyn Mode>) {
        self.mode_dots_passed = 0;
        self.mode = mode.clone();
        self.check_vblank_interrupt();
        self.set_stat_mode();
        self.check_coincidence_stat_interrupt();
        self.check_mode_stat_interrupt();
    }

    fn set_stat_mode(&mut self) {
        let code = self.mode.get_mode_number();
        let new_value = (self.memory.borrow().read(STAT_ADDRESS) & 0b11111100) | code;
        self.memory.borrow_mut().write(STAT_ADDRESS, new_value);
    }

    fn check_mode_stat_interrupt(&mut self) {
        if self.mode.get_mode_number() == 3 {
            return;
        }
        let stat_value = self.memory.borrow().read(STAT_ADDRESS);
        let matching_mode_bit = 0b00001000 << self.mode.get_mode_number();
        let interrupt = self.memory.borrow().read(IF_ADDRESS) | 0b00000010;

        if stat_value & matching_mode_bit != 0 {
            self.memory.borrow_mut().write(IF_ADDRESS, interrupt);
        }
    }

    fn check_coincidence_stat_interrupt(&mut self) {
        let stat_value = self.memory.borrow().read(STAT_ADDRESS);
        let interrupt = self.memory.borrow().read(IF_ADDRESS) | 0b00000010;
        let lyc_equals_ly = (stat_value & 0b01000100) == 68;
        if lyc_equals_ly {
            self.memory.borrow_mut().write(IF_ADDRESS, interrupt);
        }
    }

    fn check_vblank_interrupt(&mut self) {
        if self.mode.get_mode_number() == 1 {
            let if_value = self.memory.borrow().read(IF_ADDRESS);
            self.memory
                .borrow_mut()
                .write(IF_ADDRESS, if_value | 0b00000001);
        }
    }
}

trait Mode {
    fn update(&self, ppu: &mut PPU, dots: u32);
    fn transition(&self, ppu: &mut PPU);
    fn get_mode_number(&self) -> u8;
}

struct HBlank {
    dots_until_transition: u32,
}
impl Mode for HBlank {
    fn update(&self, ppu: &mut PPU, dots: u32) {
        ppu.mode_dots_passed += dots;
        if ppu.mode_dots_passed >= self.dots_until_transition {
            let leftover = ppu.mode_dots_passed - self.dots_until_transition;
            self.transition(ppu);
            ppu.update(leftover);
        }
    }

    fn transition(&self, ppu: &mut PPU) {
        let last_scanline = 143;
        if ppu.current_scanline() == last_scanline {
            ppu.set_mode(Rc::new(VBlank));
        } else {
            ppu.set_mode(Rc::new(Scan));
        }
    }

    fn get_mode_number(&self) -> u8 {
        0
    }
}

const V_BLANK_TIME: u32 = 4560;
const SCAN_TIME: u32 = 80;

struct VBlank;
impl Mode for VBlank {
    fn update(&self, ppu: &mut PPU, dots: u32) {
        ppu.mode_dots_passed += dots;
        if ppu.mode_dots_passed >= V_BLANK_TIME {
            let leftover = ppu.mode_dots_passed - V_BLANK_TIME;
            self.transition(ppu);
            ppu.update(leftover);
        }
    }

    fn transition(&self, ppu: &mut PPU) {
        ppu.set_mode(Rc::new(Scan));
        // Todo: Finish frame and start new one
    }

    fn get_mode_number(&self) -> u8 {
        1
    }
}

struct Scan;
impl Mode for Scan {
    fn update(&self, ppu: &mut PPU, dots: u32) {
        // oam scan
    }

    fn transition(&self, ppu: &mut PPU) {
        let initial_draw_time = 172;
        ppu.set_mode(Rc::new(Draw {
            dots_until_transition: initial_draw_time,
        }))
    }

    fn get_mode_number(&self) -> u8 {
        2
    }
}

struct Draw {
    dots_until_transition: u32,
}
impl Mode for Draw {
    fn update(&self, ppu: &mut PPU, dots: u32) {
        // pixel drawing
    }

    fn transition(&self, ppu: &mut PPU) {}

    fn get_mode_number(&self) -> u8 {
        3
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn get_test_ppu() -> PPU {
        let mem = Rc::new(RefCell::new(MemManager::new()));
        PPU::new(mem)
    }

    #[test]
    fn hblank_transitions_to_scan() {
        let mut ppu = get_test_ppu();
        ppu.set_mode(Rc::new(HBlank {
            dots_until_transition: 80,
        }));
        assert_eq!(ppu.mode.get_mode_number(), 0);
        ppu.update(80);
        assert_eq!(ppu.mode.get_mode_number(), 2);
    }

    #[test]
    fn hblank_does_not_transition_without_enough_cycles() {
        let mut ppu = get_test_ppu();
        ppu.set_mode(Rc::new(HBlank {
            dots_until_transition: 80,
        }));
        assert_eq!(ppu.mode.get_mode_number(), 0);
        ppu.update(79);
        assert_eq!(ppu.mode.get_mode_number(), 0);
    }

    #[test]
    fn hblank_transitions_to_vblank_at_last_line() {
        let mut ppu = get_test_ppu();
        ppu.set_mode(Rc::new(HBlank {
            dots_until_transition: 80,
        }));
        assert_eq!(ppu.mode.get_mode_number(), 0);
        ppu.memory.borrow_mut().write(LY_ADDRESS, 143);
        ppu.update(80);
        assert_eq!(ppu.mode.get_mode_number(), 1);
    }

    #[test]
    fn vblank_transitions_to_scan() {
        let mut ppu = get_test_ppu();
        ppu.set_mode(Rc::new(VBlank));
        assert_eq!(ppu.mode.get_mode_number(), 1);
        ppu.update(V_BLANK_TIME);
        assert_eq!(ppu.mode.get_mode_number(), 2);
    }

    #[test]
    fn leftover_cycles_are_carried_over_across_transitions() {
        let mut ppu = get_test_ppu();
        ppu.set_mode(Rc::new(HBlank {
            dots_until_transition: 80,
        }));
        assert_eq!(ppu.mode.get_mode_number(), 0);
        ppu.memory.borrow_mut().write(LY_ADDRESS, 143);
        ppu.update(90);
        assert_eq!(ppu.mode.get_mode_number(), 1);
        ppu.update(V_BLANK_TIME - 10);
        assert_eq!(ppu.mode.get_mode_number(), 2);
    }

    #[test]
    fn holds_cycles_between_separate_updates() {
        let mut ppu = get_test_ppu();
        ppu.set_mode(Rc::new(HBlank {
            dots_until_transition: 80,
        }));
        assert_eq!(ppu.mode.get_mode_number(), 0);
        ppu.update(40);
        assert_eq!(ppu.mode.get_mode_number(), 0);
        ppu.update(40);
        assert_eq!(ppu.mode.get_mode_number(), 2);
    }
}
