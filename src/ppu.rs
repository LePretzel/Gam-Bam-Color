use std::{cell::RefCell, collections::VecDeque, rc::Rc};

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

type Pixel = u16;

pub struct PPU {
    mode: Rc<RefCell<dyn Mode>>,
    memory: Rc<RefCell<MemManager>>,
    current_frame: Vec<Pixel>,
    completed_frame: Vec<Pixel>,
    extra_dots: u32,
    mode_dots_passed: u32,
    objects_on_scanline: Vec<u16>,
    object_pixel_queue: VecDeque<Pixel>,
    background_pixel_queue: VecDeque<Pixel>,
}

impl PPU {
    pub fn new(memory: Rc<RefCell<MemManager>>) -> Self {
        let initial_mode = Rc::new(RefCell::new(Scan));
        let mut ppu = PPU {
            mode: initial_mode.clone(),
            memory: memory.clone(),
            current_frame: Vec::new(),
            completed_frame: Vec::new(),
            extra_dots: 0,
            mode_dots_passed: 0,
            objects_on_scanline: Vec::new(),
            object_pixel_queue: VecDeque::new(),
            background_pixel_queue: VecDeque::new(),
        };
        ppu.set_mode(initial_mode.clone());

        ppu.memory.borrow_mut().write(LCDC_ADDRESS, 0x91);
        ppu.memory.borrow_mut().write(BGP_ADDRESS, 0xFC);
        ppu
    }

    pub fn update(&mut self, dots: u32) {
        let m = self.mode.clone();
        m.borrow_mut().update(self, dots);
    }

    fn current_scanline(&self) -> u8 {
        self.memory.borrow().read(LY_ADDRESS)
    }

    fn set_scanline(&mut self, value: u8) {
        self.memory.borrow_mut().write(LY_ADDRESS, value);
        self.check_coincidence_stat_interrupt();
    }

    fn set_mode(&mut self, mode: Rc<RefCell<dyn Mode>>) {
        self.mode_dots_passed = 0;
        self.mode = mode.clone();
        self.check_vblank_interrupt();
        self.set_stat_mode();
        self.check_coincidence_stat_interrupt();
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
        let stat_value = self.memory.borrow().read(STAT_ADDRESS);
        let interrupt = self.memory.borrow().read(IF_ADDRESS) | 0b00000010;
        let lyc_equals_ly = (stat_value & 0b01000100) == 68;
        if lyc_equals_ly {
            self.memory.borrow_mut().write(IF_ADDRESS, interrupt);
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

trait Mode {
    fn update(&mut self, ppu: &mut PPU, dots: u32);
    fn transition(&self, ppu: &mut PPU);
    fn get_mode_number(&self) -> u8;
}

struct HBlank {
    dots_until_transition: u32,
}
impl Mode for HBlank {
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
        if ppu.current_scanline() == last_scanline {
            ppu.set_mode(Rc::new(RefCell::new(VBlank)));
        } else {
            ppu.set_mode(Rc::new(RefCell::new(Scan)));
            ppu.clear_pixel_queues();
            ppu.set_scanline(ppu.current_scanline() + 1);
        }
    }

    fn get_mode_number(&self) -> u8 {
        0
    }
}

const V_BLANK_TIME: u32 = 4560;
const SCAN_TIME: u32 = 80;
const DRAW_PLUS_HBLANK_TIME: u32 = 376;

struct VBlank;
impl Mode for VBlank {
    fn update(&mut self, ppu: &mut PPU, dots: u32) {
        ppu.mode_dots_passed += dots;
        if ppu.mode_dots_passed >= V_BLANK_TIME {
            let leftover = ppu.mode_dots_passed - V_BLANK_TIME;
            self.transition(ppu);
            ppu.update(leftover);
        }
    }

    fn transition(&self, ppu: &mut PPU) {
        ppu.set_mode(Rc::new(RefCell::new(Scan)));
        ppu.clear_pixel_queues();
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
        let object_memory_size = 4;
        for address in (oam_range).step_by(object_memory_size) {
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
    fn update(&mut self, ppu: &mut PPU, dots: u32) {
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
        ppu.set_mode(Rc::new(RefCell::new(Draw {
            dots_until_transition: initial_draw_time,
            fetcher: Fetcher::new(),
        })));
    }

    fn get_mode_number(&self) -> u8 {
        2
    }
}

struct Fetcher {
    tilemap_col: u8,
}

impl Fetcher {
    fn new() -> Self {
        Self { tilemap_col: 0 }
    }

    fn get_tile_index(&mut self, ppu: &PPU) -> u8 {
        let lcdc = ppu.memory.borrow().read(LCDC_ADDRESS);

        let wx = ppu.memory.borrow().read(WX_ADDRESS).wrapping_sub(7);
        let wy = ppu.memory.borrow().read(WY_ADDRESS);
        let is_window_tile = ppu.current_scanline() >= wy / 8 && (self.tilemap_col >= wx / 8);
        let window_enabled = lcdc & 0b00100000 != 0;
        let uses_window = window_enabled && is_window_tile;

        let scx = ppu.memory.borrow().read(SCX_ADDRESS);
        let scy = ppu.memory.borrow().read(SCY_ADDRESS);
        let screen_offset_x = if uses_window { 0 } else { scx / 8 };
        let screen_offset_y = if uses_window { 0 } else { scy };

        let bg_switch_tilemap = lcdc & 0b00001000 != 0;
        let wd_switch_tilemap = lcdc & 0b01000000 != 0;
        let tilemap_start: u16 =
            if (uses_window && wd_switch_tilemap) || (!uses_window && bg_switch_tilemap) {
                0x9C00
            } else {
                0x9800
            };

        let tilemap_row_width: u16 = 32;
        let tilemap_x = ((screen_offset_x + self.tilemap_col) & 0x1F) as u16;
        let tilemap_y = (ppu.current_scanline().wrapping_add(screen_offset_y) / 8) as u16;
        let tile_position = tilemap_y * tilemap_row_width + tilemap_x;

        self.tilemap_col += 1;
        ppu.memory.borrow().read(tilemap_start + tile_position)
    }

    fn get_tile_data(&self, ppu: &PPU, index: u8, is_object: bool) -> u8 {
        let lcdc = ppu.memory.borrow().read(LCDC_ADDRESS);
        let signed_addressing = !is_object && lcdc & 0b00010000 == 0;
        let base_address = if signed_addressing {
            let signed_index: i32 = if index > 127 {
                -((index as i32) - 127)
            } else {
                index as i32
            };
            (0x9000 + signed_index) as u16
        } else {
            0x8000 + index as u16
        };
        let row_offset = (ppu.current_scanline() % 8) * 2;
        ppu.memory.borrow().read(base_address + row_offset as u16)
    }
}

struct Draw {
    dots_until_transition: u32,
    fetcher: Fetcher,
}
impl Mode for Draw {
    fn update(&mut self, ppu: &mut PPU, dots: u32) {}

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
    use super::*;

    fn get_test_ppu() -> PPU {
        let mem = Rc::new(RefCell::new(MemManager::new()));
        PPU::new(mem.clone())
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

    #[test]
    fn scan_transitions_to_draw() {
        let mut ppu = get_test_ppu();
        ppu.set_mode(Rc::new(RefCell::new(Scan)));
        ppu.update(80);
        assert_eq!(ppu.mode.borrow().get_mode_number(), 3);
    }

    // #[test]
    // fn draw_transitions_to_hblank

    // Fetching background tile indices
    #[test]
    fn fetcher_gets_index_of_first_tile() {
        let ppu = get_test_ppu();
        ppu.memory.borrow_mut().write(0x9800, 0xAA);
        let mut fetcher = Fetcher::new();
        assert_eq!(fetcher.get_tile_index(&ppu), 0xAA);
    }

    #[test]
    fn fetcher_gets_index_for_second_tile() {
        let ppu = get_test_ppu();
        ppu.memory.borrow_mut().write(0x9801, 0xBB);
        let mut fetcher = Fetcher::new();
        fetcher.tilemap_col += 1;
        assert_eq!(fetcher.get_tile_index(&ppu), 0xBB);
    }

    #[test]
    fn fetcher_gets_index_for_second_row() {
        let ppu = get_test_ppu();
        ppu.memory.borrow_mut().write(LY_ADDRESS, 0x08);
        ppu.memory.borrow_mut().write(0x9820, 0xCC);
        let mut fetcher = Fetcher::new();
        assert_eq!(fetcher.get_tile_index(&ppu), 0xCC);
    }

    #[test]
    fn fetcher_gets_index_for_final_row() {
        let ppu = get_test_ppu();
        ppu.memory.borrow_mut().write(LY_ADDRESS, 143);
        ppu.memory.borrow_mut().write(0x9800 + 32 * 17, 0xCC);
        let mut fetcher = Fetcher::new();
        assert_eq!(fetcher.get_tile_index(&ppu), 0xCC);
    }

    #[test]
    fn fetcher_gets_index_for_final_column() {
        let ppu = get_test_ppu();
        ppu.memory.borrow_mut().write(WX_ADDRESS, 255);
        ppu.memory.borrow_mut().write(0x9807, 0xCC);
        let mut fetcher = Fetcher::new();
        fetcher.tilemap_col = 7;
        assert_eq!(fetcher.get_tile_index(&ppu), 0xCC);
    }

    #[test]
    fn background_index_fetching_works_with_second_tilemap() {
        let ppu = get_test_ppu();
        ppu.memory.borrow_mut().write(0x9C00, 0xAA);
        ppu.memory.borrow_mut().write(LCDC_ADDRESS, 0b10011001);
        let mut fetcher = Fetcher::new();
        assert_eq!(fetcher.get_tile_index(&ppu), 0xAA);
    }

    #[test]
    fn fetcher_gets_correct_index_with_screen_scrolled() {
        let ppu = get_test_ppu();
        ppu.memory.borrow_mut().write(SCX_ADDRESS, 95);
        ppu.memory.borrow_mut().write(SCY_ADDRESS, 111);
        ppu.memory.borrow_mut().write(0x99AB, 0xAA);
        ppu.memory.borrow_mut().write(LCDC_ADDRESS, 0b10010001);
        let mut fetcher = Fetcher::new();
        assert_eq!(fetcher.get_tile_index(&ppu), 0xAA);
    }

    #[test]
    fn screen_wraps_around_horizontal() {
        let ppu = get_test_ppu();
        ppu.memory.borrow_mut().write(WY_ADDRESS, 255);
        ppu.memory.borrow_mut().write(0x9800, 0xDD);
        ppu.memory.borrow_mut().write(SCX_ADDRESS, 80);
        let mut fetcher = Fetcher::new();
        fetcher.tilemap_col = 22;
        assert_eq!(fetcher.get_tile_index(&ppu), 0xDD);
    }

    #[test]
    fn screen_wraps_around_vertical() {
        let ppu = get_test_ppu();
        ppu.memory.borrow_mut().write(LY_ADDRESS, 206);
        ppu.memory.borrow_mut().write(SCY_ADDRESS, 50);
        ppu.memory.borrow_mut().write(0x9800, 0xDD);
        let mut fetcher = Fetcher::new();
        assert_eq!(fetcher.get_tile_index(&ppu), 0xDD);
    }

    // Fetching window tile indices
    #[test]
    fn fetcher_gets_window_index_first_row_first_column() {
        let ppu = get_test_ppu();
        ppu.memory.borrow_mut().write(WX_ADDRESS, 7);
        ppu.memory.borrow_mut().write(WY_ADDRESS, 0);
        ppu.memory.borrow_mut().write(0x9800, 0xAA);
        ppu.memory.borrow_mut().write(LCDC_ADDRESS, 0b10110001);
        let mut fetcher = Fetcher::new();
        assert_eq!(fetcher.get_tile_index(&ppu), 0xAA);
    }

    #[test]
    fn fetcher_gets_window_index_second_row_first_column() {
        let ppu = get_test_ppu();
        ppu.memory.borrow_mut().write(WX_ADDRESS, 7);
        ppu.memory.borrow_mut().write(WY_ADDRESS, 0);
        ppu.memory.borrow_mut().write(LY_ADDRESS, 0x08);
        ppu.memory.borrow_mut().write(0x9820, 0xAA);
        ppu.memory.borrow_mut().write(LCDC_ADDRESS, 0b10110001);
        let mut fetcher = Fetcher::new();
        assert_eq!(fetcher.get_tile_index(&ppu), 0xAA);
    }

    #[test]
    fn fetches_two_consecutive_window_indices() {
        let ppu = get_test_ppu();
        ppu.memory.borrow_mut().write(WX_ADDRESS, 7);
        ppu.memory.borrow_mut().write(WY_ADDRESS, 0);
        ppu.memory.borrow_mut().write(0x9801, 0xBA);
        ppu.memory.borrow_mut().write(LCDC_ADDRESS, 0b10110001);
        let mut fetcher = Fetcher::new();
        fetcher.get_tile_index(&ppu);
        assert_eq!(fetcher.get_tile_index(&ppu), 0xBA);
    }

    #[test]
    fn fetching_window_indices_works_with_second_tilemap() {
        let ppu = get_test_ppu();
        ppu.memory.borrow_mut().write(WX_ADDRESS, 7);
        ppu.memory.borrow_mut().write(WY_ADDRESS, 0);
        ppu.memory.borrow_mut().write(0x9C00, 0xAA);
        ppu.memory.borrow_mut().write(LCDC_ADDRESS, 0b11110001);
        let mut fetcher = Fetcher::new();
        assert_eq!(fetcher.get_tile_index(&ppu), 0xAA);
    }

    #[test]
    fn gets_tile_data_first_row_first_byte() {
        let ppu = get_test_ppu();
        ppu.memory.borrow_mut().write(0x8000, 0x11);
        let fetcher = Fetcher::new();
        let result = fetcher.get_tile_data(&ppu, 0, false);
        assert_eq!(result, 0x11);
    }

    #[test]
    fn gets_tile_data_first_row_second_byte() {
        let ppu = get_test_ppu();
        ppu.memory.borrow_mut().write(0x8001, 0x11);
        let fetcher = Fetcher::new();
        let result = fetcher.get_tile_data(&ppu, 1, false);
        assert_eq!(result, 0x11);
    }

    #[test]
    fn gets_tile_data_second_row_first_byte() {
        let ppu = get_test_ppu();
        ppu.memory.borrow_mut().write(0x8002, 0x11);
        ppu.memory.borrow_mut().write(LY_ADDRESS, 0x01);
        let fetcher = Fetcher::new();
        let result = fetcher.get_tile_data(&ppu, 0, false);
        assert_eq!(result, 0x11);
    }

    #[test]
    fn gets_tile_data_seventh_row_second_byte() {
        let ppu = get_test_ppu();
        ppu.memory.borrow_mut().write(0x800F, 0x11);
        ppu.memory.borrow_mut().write(LY_ADDRESS, 0x07);
        let fetcher = Fetcher::new();
        let result = fetcher.get_tile_data(&ppu, 1, false);
        assert_eq!(result, 0x11);
    }
}
