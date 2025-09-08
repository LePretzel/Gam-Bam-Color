use std::{cell::RefCell, collections::VecDeque, rc::Rc};

use crate::mem_manager::MemManager;
use crate::memory::Memory;

use crate::fetcher::{BackgroundFetcher, SpriteFetcher};

use crate::registers::{
    BCPD_ADDRESS, BCPS_ADDRESS, BGP_ADDRESS, IF_ADDRESS, LCDC_ADDRESS, LYC_ADDRESS, LY_ADDRESS,
    OCPD_ADDRESS, OCPS_ADDRESS, SCX_ADDRESS, STAT_ADDRESS,
};

const V_BLANK_TIME: u32 = 4560;
const SCAN_TIME: u32 = 80;
const DRAW_PLUS_HBLANK_TIME: u32 = 376;
const DOTS_PER_FRAME: u32 = 70224;
const DOTS_PER_SCANLINE: u32 = 456;

#[derive(Clone, Copy)]
pub(crate) struct ObjectPixel {
    pub color: u8,
    pub palette: u8,
    pub sprite_prio: u8,
    pub bg_prio: bool,
}

#[derive(Clone, Copy)]
pub(crate) struct BackgroundPixel {
    pub color: u8,
    pub palette: u8,
}

type RenderedPixel = u8;

// Todo: Implement ppu vram blocking
// Todo: Implement window rendering penalty
// Todo: More complex behavior for cgb palette access
// Todo: Original gameboy compatibility
pub struct PPU {
    mode: Rc<RefCell<dyn PPUMode>>,
    pub(crate) memory: Rc<RefCell<MemManager>>,
    current_frame: Vec<RenderedPixel>,
    completed_frame: Vec<RenderedPixel>,
    mode_dots_passed: u32,
    pub(crate) objects_on_scanline: Vec<u16>,
    pub(crate) object_pixel_queue: VecDeque<ObjectPixel>,
    pub(crate) background_pixel_queue: VecDeque<BackgroundPixel>,
    pub(crate) screen_x: u8,
}

impl PPU {
    pub fn new(memory: Rc<RefCell<MemManager>>) -> Self {
        let mut ppu = PPU::new_test(memory);
        // Needed because emulator starts at pc = 0x0100 instead of actual hardware that starts at pc = 0x0000
        ppu.update(DOTS_PER_SCANLINE * 147 + 180);
        ppu
    }

    pub(crate) fn new_test(memory: Rc<RefCell<MemManager>>) -> Self {
        let initial_mode = Rc::new(RefCell::new(Scan));
        let mut ppu = PPU {
            mode: initial_mode.clone(),
            memory: memory.clone(),
            current_frame: Vec::new(),
            completed_frame: Vec::new(),
            mode_dots_passed: 0,
            objects_on_scanline: Vec::new(),
            object_pixel_queue: VecDeque::with_capacity(16),
            background_pixel_queue: VecDeque::with_capacity(16),
            screen_x: 0,
        };
        ppu.set_mode(initial_mode);

        ppu.memory.borrow_mut().write(LCDC_ADDRESS, 0x91);
        ppu.memory.borrow_mut().write(BGP_ADDRESS, 0xFC);
        ppu
    }

    pub fn update(&mut self, dots: u32) {
        let m = self.mode.clone();
        m.borrow_mut().update(self, dots);
    }

    pub fn get_frame(&self) -> Vec<u8> {
        self.completed_frame.clone()
    }

    pub fn get_current_scanline(&self) -> u8 {
        self.memory.borrow().read(LY_ADDRESS)
    }

    fn set_scanline(&mut self, value: u8) {
        self.memory.borrow_mut().write(LY_ADDRESS, value);
        self.check_coincidence_stat_interrupt();
    }

    fn set_mode(&mut self, mode: Rc<RefCell<dyn PPUMode>>) {
        self.mode_dots_passed = 0;
        self.mode = mode.clone();
        self.check_vblank_interrupt();
        self.set_stat_mode();
        self.check_mode_stat_interrupt();
    }

    fn set_stat_mode(&mut self) {
        let code = self.mode.borrow().get_mode_number();
        let new_value = (self.memory.borrow().read(STAT_ADDRESS) & 0b11111100) | code;
        self.memory.borrow_mut().write(STAT_ADDRESS, new_value);
    }

    fn clear_pixel_queues(&mut self) {
        self.object_pixel_queue.clear();
        self.background_pixel_queue.clear();
    }

    fn check_mode_stat_interrupt(&mut self) {
        if self.mode.borrow().get_mode_number() == 3 {
            return;
        }
        let stat_value = self.memory.borrow().read(STAT_ADDRESS);
        let matching_mode_bit = 0b00001000 << self.mode.borrow().get_mode_number();
        let interrupt = self.memory.borrow().read(IF_ADDRESS) | 0b00000010;

        if stat_value & matching_mode_bit != 0 {
            self.memory.borrow_mut().write(IF_ADDRESS, interrupt);
        }
    }

    fn check_coincidence_stat_interrupt(&mut self) {
        let current_scanline = self.get_current_scanline();
        let lyc = self.memory.borrow().read(LYC_ADDRESS);
        let mut stat_value = self.memory.borrow().read(STAT_ADDRESS);
        if current_scanline == lyc {
            stat_value |= 0b00000100;
        } else {
            stat_value &= 0b11111011;
        }

        self.memory.borrow_mut().write(STAT_ADDRESS, stat_value);

        let coincidence_enabled = stat_value & 0b01000000 != 0;
        if coincidence_enabled && current_scanline == lyc {
            let if_value = self.memory.borrow().read(IF_ADDRESS);
            self.memory
                .borrow_mut()
                .write(IF_ADDRESS, if_value | 0b00000010);
            // println!(
            //     "coincide at {}. WX is {} WY is {}.",
            //     self.get_current_scanline(),
            //     self.memory.borrow().read(WX_ADDRESS),
            //     self.memory.borrow().read(WY_ADDRESS)
            // );
        }
    }

    fn check_vblank_interrupt(&mut self) {
        if self.mode.borrow().get_mode_number() == 1 {
            let if_value = self.memory.borrow().read(IF_ADDRESS);
            self.memory
                .borrow_mut()
                .write(IF_ADDRESS, if_value | 0b00000001);
        }
    }
}

trait PPUMode {
    fn update(&mut self, ppu: &mut PPU, dots: u32);
    fn transition(&self, ppu: &mut PPU);
    fn get_mode_number(&self) -> u8;
}

pub(crate) struct HBlank {
    dots_until_transition: u32,
}
impl PPUMode for HBlank {
    fn update(&mut self, ppu: &mut PPU, dots: u32) {
        ppu.mode_dots_passed += dots;
        if ppu.mode_dots_passed >= self.dots_until_transition {
            let leftover = ppu.mode_dots_passed - self.dots_until_transition;
            self.transition(ppu);
            ppu.update(leftover);
        }
    }

    fn transition(&self, ppu: &mut PPU) {
        let last_scanline = 143;
        if ppu.get_current_scanline() == last_scanline {
            ppu.set_mode(Rc::new(RefCell::new(VBlank)));
        } else {
            ppu.set_mode(Rc::new(RefCell::new(Scan)));
            ppu.clear_pixel_queues();
            ppu.set_scanline(ppu.get_current_scanline() + 1);
        }
    }

    fn get_mode_number(&self) -> u8 {
        0
    }
}

pub(crate) struct VBlank;
impl PPUMode for VBlank {
    fn update(&mut self, ppu: &mut PPU, dots: u32) {
        ppu.mode_dots_passed += dots;
        // Update LY if a scanline's worth of dots have passed
        if ppu.mode_dots_passed % DOTS_PER_SCANLINE < dots {
            ppu.set_scanline(143 + (ppu.mode_dots_passed / DOTS_PER_SCANLINE) as u8);
        }
        if ppu.mode_dots_passed >= V_BLANK_TIME {
            let leftover = ppu.mode_dots_passed - V_BLANK_TIME;
            self.transition(ppu);
            ppu.update(leftover);
        }
    }

    fn transition(&self, ppu: &mut PPU) {
        ppu.set_mode(Rc::new(RefCell::new(Scan)));
        ppu.set_scanline(0);
        ppu.completed_frame = ppu.current_frame.clone();
        ppu.current_frame.clear();
    }

    fn get_mode_number(&self) -> u8 {
        1
    }
}

pub(crate) struct Scan;
impl Scan {
    fn select_objects(&self, ppu: &mut PPU) {
        ppu.objects_on_scanline.clear();
        let large_objects_enabled = ppu.memory.borrow().read(LCDC_ADDRESS) & 0b00000100 == 4;
        let oam_range = 0xFE00..=0xFE9F;
        let object_memory_size = 4;
        for address in (oam_range).step_by(object_memory_size) {
            if ppu.objects_on_scanline.len() == 10 {
                break;
            }
            let object_y = ppu.memory.borrow().read(address);

            let object_attrs = ppu.memory.borrow().read(address + 3);
            let is_large_object = object_attrs & 0b01000000 != 0;
            let object_size = if large_objects_enabled { 16 } else { 8 };
            let object_top = object_y as i8 - 16;
            let object_bottom = object_top + object_size;
            let object_pixel_range = object_top..object_bottom;

            if object_pixel_range.contains(&(ppu.get_current_scanline() as i8)) {
                ppu.objects_on_scanline.push(address);
            }
        }
    }
}

impl PPUMode for Scan {
    fn update(&mut self, ppu: &mut PPU, dots: u32) {
        ppu.mode_dots_passed += dots;
        if ppu.mode_dots_passed >= SCAN_TIME {
            self.select_objects(ppu);
            let leftover = ppu.mode_dots_passed - SCAN_TIME;
            self.transition(ppu);
            ppu.update(leftover);
        }
    }

    fn transition(&self, ppu: &mut PPU) {
        ppu.clear_pixel_queues();
        let new_mode = Rc::new(RefCell::new(Draw::new()));
        // Perform one fetch early for timing purposes
        for _ in 0..6 {
            new_mode.borrow_mut().bg_fetcher.tick(ppu);
        }
        ppu.set_mode(new_mode);
        ppu.screen_x = 0;
    }

    fn get_mode_number(&self) -> u8 {
        2
    }
}

pub(crate) struct Draw {
    pub(crate) bg_fetcher: BackgroundFetcher,
    pub(crate) obj_fetcher: SpriteFetcher,
}

impl Draw {
    pub fn new() -> Self {
        Draw {
            bg_fetcher: BackgroundFetcher::new(),
            obj_fetcher: SpriteFetcher::new(),
        }
    }

    pub(crate) fn tick(&mut self, ppu: &mut PPU) -> bool {
        ppu.mode_dots_passed += 1;

        if self.obj_fetcher.has_sprite_queued() {
            self.obj_fetcher.tick(ppu);
            return true;
        }

        if ppu.background_pixel_queue.len() > 8 {
            // Throw away the pixels that are cut off by screen scroll
            if ppu.mode_dots_passed <= (ppu.memory.borrow().read(SCX_ADDRESS) % 8) as u32 {
                let _ = ppu.background_pixel_queue.pop_front();
                if !ppu.background_pixel_queue.is_empty() {
                    let _ = ppu.object_pixel_queue.pop_front();
                }
            } else if ppu.screen_x < 160 {
                // Stop pushing to lcd at x >= 160 but keep running for 6 more dots
                // to simulate the fetch of the final tile that would be offscreen
                self.push_pixel_to_lcd(ppu);
            }

            // Check for objects in this position before moving on
            ppu.objects_on_scanline.reverse();
            for object_address in &ppu.objects_on_scanline {
                let object_end = ppu.memory.borrow().read(object_address + 1);
                if object_end < 8 {
                    continue;
                }
                let object_start = object_end - 8;
                if object_start == ppu.screen_x {
                    self.obj_fetcher.start_fetch(*object_address);
                }
            }
            ppu.screen_x += 1;

            if ppu.screen_x >= 166 {
                return false;
            }
        }
        self.bg_fetcher.tick(ppu);
        return true;
    }

    fn render_object_pixel(&self, ppu: &mut PPU, pixel: ObjectPixel) -> Vec<u8> {
        let color_index = (4 * pixel.palette + pixel.color) * 2;
        let ocps_value = ppu.memory.borrow().read(OCPS_ADDRESS);
        ppu.memory.borrow_mut().write(OCPS_ADDRESS, color_index);
        let high_byte = ppu.memory.borrow().read(OCPD_ADDRESS);
        ppu.memory.borrow_mut().write(OCPS_ADDRESS, color_index + 1);
        let low_byte = ppu.memory.borrow().read(OCPD_ADDRESS);
        ppu.memory.borrow_mut().write(OCPS_ADDRESS, ocps_value);
        vec![high_byte, low_byte]
    }

    fn render_background_pixel(&self, ppu: &mut PPU, pixel: BackgroundPixel) -> Vec<u8> {
        let color_index = (4 * pixel.palette + pixel.color) * 2;
        let bcps_value = ppu.memory.borrow().read(BCPS_ADDRESS);
        ppu.memory.borrow_mut().write(BCPS_ADDRESS, color_index);
        let high_byte = ppu.memory.borrow().read(BCPD_ADDRESS);
        ppu.memory.borrow_mut().write(BCPS_ADDRESS, color_index + 1);
        let low_byte = ppu.memory.borrow().read(BCPD_ADDRESS);
        ppu.memory.borrow_mut().write(BCPS_ADDRESS, bcps_value);
        vec![high_byte, low_byte]
    }

    fn push_pixel_to_lcd(&self, ppu: &mut PPU) {
        assert!(ppu.background_pixel_queue.len() > 8);
        let bg_pixel = ppu.background_pixel_queue.pop_front();
        let obj_pixel = ppu.object_pixel_queue.pop_front();
        if let Some(pixel) = obj_pixel {
            if !pixel.bg_prio && pixel.color != 0 {
                let rendered_pixel = self.render_object_pixel(ppu, pixel);
                for i in rendered_pixel.iter() {
                    ppu.current_frame.push(*i);
                }
                return;
            }
        }
        let rendered_pixel = self.render_background_pixel(ppu, bg_pixel.unwrap());
        for i in rendered_pixel.iter() {
            ppu.current_frame.push(*i);
        }
    }
}

impl PPUMode for Draw {
    fn update(&mut self, ppu: &mut PPU, dots: u32) {
        for used_dots in 1..=dots {
            let currently_drawing = self.tick(ppu);
            if !currently_drawing {
                let leftover = dots - used_dots;
                self.transition(ppu);
                ppu.update(leftover);
                break;
            }
        }
    }

    fn transition(&self, ppu: &mut PPU) {
        let hblank_time = DRAW_PLUS_HBLANK_TIME - ppu.mode_dots_passed;
        ppu.set_mode(Rc::new(RefCell::new(HBlank {
            dots_until_transition: hblank_time,
        })));
    }

    fn get_mode_number(&self) -> u8 {
        3
    }
}

#[cfg(test)]
mod tests {
    use crate::registers::SCY_ADDRESS;

    use super::*;

    fn get_test_ppu() -> PPU {
        let mem = Rc::new(RefCell::new(MemManager::new()));
        PPU::new_test(mem.clone())
    }

    fn set_obj_y_pos(ppu: &mut PPU, obj_index: u8, scanline: u8) {
        assert!(obj_index < 40);
        let obj_y_address = 0xFE00 + ((obj_index * 4) as u16);
        ppu.memory.borrow_mut().write(obj_y_address, scanline);
    }

    #[test]
    fn hblank_transitions_to_scan() {
        let mut ppu = get_test_ppu();
        ppu.set_mode(Rc::new(RefCell::new(HBlank {
            dots_until_transition: 80,
        })));
        assert_eq!(ppu.mode.borrow().get_mode_number(), 0);
        ppu.update(80);
        assert_eq!(ppu.mode.borrow().get_mode_number(), 2);
    }

    #[test]
    fn hblank_does_not_transition_without_enough_cycles() {
        let mut ppu = get_test_ppu();
        ppu.set_mode(Rc::new(RefCell::new(HBlank {
            dots_until_transition: 80,
        })));
        assert_eq!(ppu.mode.borrow().get_mode_number(), 0);
        ppu.update(79);
        assert_eq!(ppu.mode.borrow().get_mode_number(), 0);
    }

    #[test]
    fn hblank_transitions_to_vblank_at_last_line() {
        let mut ppu = get_test_ppu();
        ppu.set_mode(Rc::new(RefCell::new(HBlank {
            dots_until_transition: 80,
        })));
        assert_eq!(ppu.mode.borrow().get_mode_number(), 0);
        ppu.memory.borrow_mut().write(LY_ADDRESS, 143);
        ppu.update(80);
        assert_eq!(ppu.mode.borrow().get_mode_number(), 1);
    }

    #[test]
    fn vblank_updates_ly_with_exact_dots() {
        let mut ppu = get_test_ppu();
        ppu.set_mode(Rc::new(RefCell::new(VBlank)));
        ppu.memory.borrow_mut().write(LY_ADDRESS, 143);
        let ly_initial = ppu.memory.borrow().read(LY_ADDRESS);
        ppu.update(456);
        assert_eq!(ppu.memory.borrow().read(LY_ADDRESS), ly_initial + 1);
    }

    #[test]
    fn vblank_does_not_update_ly_without_enough_dots() {
        let mut ppu = get_test_ppu();
        ppu.set_mode(Rc::new(RefCell::new(VBlank)));
        let ly_initial = ppu.memory.borrow().read(LY_ADDRESS);
        ppu.update(455);
        assert_eq!(ppu.memory.borrow().read(LY_ADDRESS), ly_initial);
    }

    #[test]
    fn vblank_updates_ly_with_extra_dots() {
        let mut ppu = get_test_ppu();
        ppu.set_mode(Rc::new(RefCell::new(VBlank)));
        ppu.memory.borrow_mut().write(LY_ADDRESS, 143);
        let ly_initial = ppu.memory.borrow().read(LY_ADDRESS);
        ppu.update(800);
        assert_eq!(ppu.memory.borrow().read(LY_ADDRESS), ly_initial + 1);
    }

    #[test]
    fn vblank_transitions_to_scan() {
        let mut ppu = get_test_ppu();
        ppu.set_mode(Rc::new(RefCell::new(VBlank)));
        assert_eq!(ppu.mode.borrow().get_mode_number(), 1);
        ppu.update(V_BLANK_TIME);
        assert_eq!(ppu.mode.borrow().get_mode_number(), 2);
    }

    #[test]
    fn leftover_cycles_are_carried_over_across_transitions() {
        let mut ppu = get_test_ppu();
        ppu.set_mode(Rc::new(RefCell::new(HBlank {
            dots_until_transition: 80,
        })));
        assert_eq!(ppu.mode.borrow().get_mode_number(), 0);
        ppu.memory.borrow_mut().write(LY_ADDRESS, 143);
        ppu.update(90);
        assert_eq!(ppu.mode.borrow().get_mode_number(), 1);
        ppu.update(V_BLANK_TIME - 10);
        assert_eq!(ppu.mode.borrow().get_mode_number(), 2);
    }

    #[test]
    fn holds_cycles_between_separate_updates() {
        let mut ppu = get_test_ppu();
        ppu.set_mode(Rc::new(RefCell::new(HBlank {
            dots_until_transition: 80,
        })));
        assert_eq!(ppu.mode.borrow().get_mode_number(), 0);
        ppu.update(40);
        assert_eq!(ppu.mode.borrow().get_mode_number(), 0);
        ppu.update(40);
        assert_eq!(ppu.mode.borrow().get_mode_number(), 2);
    }

    #[test]
    fn scan_mode_finds_an_object_on_scanline() {
        let mut ppu = get_test_ppu();
        ppu.memory.borrow_mut().write(LCDC_ADDRESS, 0b00000100);
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
        ppu.set_mode(Rc::new(RefCell::new(Scan)));
        ppu.update(80);
        assert_eq!(ppu.objects_on_scanline.len(), 1);
    }

    #[test]
    fn finding_object_lengthens_draw() {
        let mut ppu = get_test_ppu();
        let mut ref_ppu = get_test_ppu();
        ppu.memory.borrow_mut().write(LCDC_ADDRESS, 0x95);
        set_obj_y_pos(&mut ppu, 0, 2);
        ppu.update(80);
        ref_ppu.update(80);
        assert_eq!(
            ppu.mode.borrow().get_mode_number(),
            ref_ppu.mode.borrow().get_mode_number()
        );
        assert_eq!(ppu.mode.borrow().get_mode_number(), 3);
        assert_eq!(ppu.objects_on_scanline.len(), 1);
        assert_eq!(ref_ppu.objects_on_scanline.len(), 0);
        ppu.update(172);
        ref_ppu.update(172);
        assert_ne!(
            ppu.mode.borrow().get_mode_number(),
            ref_ppu.mode.borrow().get_mode_number()
        );
    }

    #[test]
    fn selects_object_if_only_first_row_is_on_scanline() {
        let mut ppu = get_test_ppu();
        ppu.memory.borrow_mut().write(LCDC_ADDRESS, 0b00000100);
        set_obj_y_pos(&mut ppu, 0, 16);
        ppu.update(80);
        assert_eq!(ppu.objects_on_scanline.len(), 1);
    }

    #[test]
    fn scan_transitions_to_draw() {
        let mut ppu = get_test_ppu();
        ppu.set_mode(Rc::new(RefCell::new(Scan)));
        ppu.update(80);
        assert_eq!(ppu.mode.borrow().get_mode_number(), 3);
    }

    #[test]
    fn draw_transitions_to_hblank() {
        let mut ppu = get_test_ppu();
        ppu.set_mode(Rc::new(RefCell::new(Scan)));
        ppu.update(80);
        assert_eq!(ppu.mode.borrow().get_mode_number(), 3);
        ppu.update(172);
        assert_eq!(ppu.mode.borrow().get_mode_number(), 0);
    }

    #[test]
    fn draw_does_not_transition_to_hblank_without_enough_dots() {
        let mut ppu = get_test_ppu();
        ppu.set_mode(Rc::new(RefCell::new(Draw::new())));
        ppu.update(166);
        assert_eq!(ppu.mode.borrow().get_mode_number(), 3);
    }

    #[test]
    fn screen_wraps_around_vertical() {
        let ppu = get_test_ppu();
        ppu.memory.borrow_mut().write(LY_ADDRESS, 206);
        ppu.memory.borrow_mut().write(SCY_ADDRESS, 50);
        ppu.memory.borrow_mut().write(0x9800, 0xDD);
        let mut fetcher = BackgroundFetcher::new();
        assert_eq!(fetcher.get_tile_index(&ppu), 0xDD);
    }

    #[test]
    fn screen_wraps_around_vertical_scy_is_greater_than_screen_height() {
        let ppu = get_test_ppu();
        ppu.memory.borrow_mut().write(SCY_ADDRESS, 0x98);
        ppu.memory.borrow_mut().write(0x9800, 0x30);
        ppu.memory.borrow_mut().write(LY_ADDRESS, 104);
        let mut fetcher = BackgroundFetcher::new();
        assert_eq!(fetcher.get_tile_index(&ppu), 0x30);
    }

    #[test]
    fn gets_correct_value_for_palette() {
        let mut ppu = get_test_ppu();
        let draw = Draw::new();
        // Set the first couple colors using auto increment of bcps
        ppu.memory.borrow_mut().write(BCPS_ADDRESS, 0b10001000);
        ppu.memory.borrow_mut().write(BCPD_ADDRESS, 0x35);
        ppu.memory.borrow_mut().write(BCPD_ADDRESS, 0xad);
        ppu.memory.borrow_mut().write(BCPD_ADDRESS, 0x7f);
        ppu.memory.borrow_mut().write(BCPD_ADDRESS, 0xff);
        let pixels = draw.render_background_pixel(
            &mut ppu,
            BackgroundPixel {
                palette: 1,
                color: 1,
            },
        );
        assert_eq!(pixels, vec![0x7f, 0xff]);
    }

    #[test]
    fn get_tile_index_gets_gets_lower_data_for_large_sprites() {
        let mut ppu = get_test_ppu();
        ppu.set_scanline(8);
        let lcdc = ppu.memory.borrow().read(LCDC_ADDRESS);
        ppu.memory
            .borrow_mut()
            .write(LCDC_ADDRESS, lcdc | 0b00000100);
        for i in 0x8000..=0x8020 {
            ppu.memory.borrow_mut().write(i, (i - 0x8000) as u8);
        }
        let mut fetcher = SpriteFetcher::new();
        fetcher.start_fetch(0);
        let res = fetcher.get_tile_data(&ppu, 0, false);
        assert_eq!(res, 16);
    }
}
