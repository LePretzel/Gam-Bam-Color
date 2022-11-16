use std::{cell::RefCell, rc::Rc};

use crate::memory::{MemManager, Memory};

const IF_ADDRESS: u16 = 0xFF0F;
const LCDC_ADDRESS: u16 = 0xFF40;
const STAT_ADDRESS: u16 = 0xFF41;
const SCY_ADDRESS: u16 = 0xFF42;
const SCX_ADDRESS: u16 = 0xFF43;
const LY_ADDRESS: u16 = 0xFF44;
const LYC_ADDRESS: u16 = 0xFF45;
const BGP_ADDRESS: u16 = 0xFF47;
const OBP0_ADDRESS: u16 = 0xFF48;
const OBP1_ADDRESS: u16 = 0xFF49;
const WY_ADDRESS: u16 = 0xFF4A;
const WX_ADDRESS: u16 = 0xFF4B;
const BCPS_ADDRESS: u16 = 0xFF68;
const BCPD_ADDRESS: u16 = 0xFF69;
const OCPS_ADDRESS: u16 = 0xFF6A;
const OCPD_ADDRESS: u16 = 0xFF68;

pub struct PPU {
    mode: Rc<dyn Mode>,
    memory: Rc<RefCell<MemManager>>,
    // current_frame,
    // completed_frame,
    extra_dots: u32,
    mode_dots_passed: u32,
    objects_on_scanline: Vec<u16>,
}

impl PPU {
    pub fn new(memory: Rc<RefCell<MemManager>>) -> Self {
        let initial_mode = Rc::new(Scan);
        let mut ppu = PPU {
            mode: initial_mode.clone(),
            memory: memory.clone(),
            extra_dots: 0,
            mode_dots_passed: 0,
            objects_on_scanline: Vec::new(),
        };
        ppu.set_mode(initial_mode.clone());

        ppu.memory.borrow_mut().write(LCDC_ADDRESS, 0x91);
        ppu.memory.borrow_mut().write(BGP_ADDRESS, 0xFC);
        ppu
    }

    pub fn update(&mut self, dots: u32) {
        let m = self.mode.clone();
        m.update(self, dots);
    }

    fn current_scanline(&self) -> u8 {
        self.memory.borrow().read(LY_ADDRESS)
    }

    fn set_scanline(&mut self, value: u8) {
        self.memory.borrow_mut().write(LY_ADDRESS, value);
        self.check_coincidence_stat_interrupt();
    }

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
            ppu.set_scanline(ppu.current_scanline() + 1);
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
        ppu.set_scanline(0);
        // Todo: Finish frame and start new one
    }

    fn get_mode_number(&self) -> u8 {
        1
    }
}

struct Scan;

impl Scan {
    fn select_objects(&self, ppu: &mut PPU) {
        ppu.objects_on_scanline.clear();
        let large_objects = ppu.memory.borrow().read(LCDC_ADDRESS) & 0b00000100 == 4;
        let oam_range = 0xFE00..=0xFE9F;
        for address in (oam_range).step_by(4) {
            if ppu.objects_on_scanline.len() == 10 {
                break;
            }
            let mut object_start: i8 = (ppu.memory.borrow().read(address) as i8) - 16;
            let object_size = if large_objects { 16 } else { 8 };
            let object_end = object_start + object_size;
            if object_end < 0 {
                continue;
            }
            if object_start < 0 {
                object_start = 0;
            }
            let object_pixel_range = (object_start as u8)..object_end as u8;
            if object_pixel_range.contains(&ppu.current_scanline()) {
                ppu.objects_on_scanline.push(address);
            }
        }
    }
}

impl Mode for Scan {
    fn update(&self, ppu: &mut PPU, dots: u32) {
        ppu.mode_dots_passed += dots;
        if ppu.mode_dots_passed >= SCAN_TIME {
            self.select_objects(ppu);
            let leftover = ppu.mode_dots_passed - SCAN_TIME;
            self.select_objects(ppu);
            self.transition(ppu);
            ppu.update(leftover);
        }
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
        PPU::new(mem.clone())
    }

    fn set_obj_y_pos(ppu: &mut PPU, obj_index: u8, scanline: u8) {
        assert!(obj_index >= 0 && obj_index < 40);
        let obj_y_address = 0xFE00 + ((obj_index * 4) as u16);
        ppu.memory.borrow_mut().write(obj_y_address, scanline);
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

    #[test]
    fn scan_mode_finds_an_object_on_scanline() {
        let mut ppu = get_test_ppu();
        ppu.memory.borrow_mut().write(LY_ADDRESS, 85);
        set_obj_y_pos(&mut ppu, 0, 100);
        ppu.update(80);
        assert_eq!(ppu.objects_on_scanline.len(), 1);
    }

    #[test]
    fn scan_mode_wont_find_an_object_if_there_are_none_on_scanline() {
        let mut ppu = get_test_ppu();
        ppu.memory.borrow_mut().write(LY_ADDRESS, 100);
        ppu.update(80);
        assert_eq!(ppu.objects_on_scanline.len(), 0);
    }

    #[test]
    fn scan_mode_does_not_find_object_without_enough_cycles() {
        let mut ppu = get_test_ppu();
        ppu.memory.borrow_mut().write(LY_ADDRESS, 85);
        set_obj_y_pos(&mut ppu, 0, 100);
        ppu.update(79);
        assert!(ppu.objects_on_scanline.is_empty());
    }

    #[test]
    fn objects_on_line_0_are_hidden() {
        let mut ppu = get_test_ppu();
        ppu.update(80);
        assert_eq!(ppu.objects_on_scanline.len(), 0);
    }

    #[test]
    fn large_objects_on_line_0_are_hidden() {
        let mut ppu = get_test_ppu();
        ppu.memory.borrow_mut().write(LCDC_ADDRESS, 0x95);
        ppu.update(80);
        assert_eq!(ppu.objects_on_scanline.len(), 0);
    }

    #[test]
    fn only_ten_objects_are_selected() {
        let mut ppu = get_test_ppu();
        ppu.memory.borrow_mut().write(LCDC_ADDRESS, 0x95);
        ppu.memory.borrow_mut().write(LY_ADDRESS, 85);
        for i in 0..20 {
            set_obj_y_pos(&mut ppu, i, 100);
        }
        ppu.update(80);
        assert_eq!(ppu.objects_on_scanline.len(), 10);
    }

    #[test]
    fn same_sprite_on_two_different_rows() {
        let mut ppu = get_test_ppu();
        ppu.memory.borrow_mut().write(LCDC_ADDRESS, 0x95);
        set_obj_y_pos(&mut ppu, 0, 2);
        ppu.update(80);
        assert_eq!(ppu.objects_on_scanline.len(), 1);
        ppu.memory.borrow_mut().write(LY_ADDRESS, 0);
        ppu.update(80);
        assert_eq!(ppu.objects_on_scanline.len(), 1);
    }
    #[test]
    fn selects_object_if_only_first_row_is_on_scanline() {
        let mut ppu = get_test_ppu();
        set_obj_y_pos(&mut ppu, 0, 16);
        ppu.update(80);
        assert_eq!(ppu.objects_on_scanline.len(), 1);
    }
}
