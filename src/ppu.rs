use std::{cell::RefCell, collections::VecDeque, rc::Rc};

use crate::mem_manager::MemManager;
use crate::memory::Memory;
use crate::ppu::FetcherStage::{DataHigh, DataLow, GetTile, Push};

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
const OCPD_ADDRESS: u16 = 0xFF6B;
const VBK_ADDRESS: u16 = 0xFF4F;

const V_BLANK_TIME: u32 = 4560;
const SCAN_TIME: u32 = 80;
const DRAW_PLUS_HBLANK_TIME: u32 = 376;
const DOTS_PER_FRAME: u32 = 70224;
const DOTS_PER_SCANLINE: u32 = 456;

#[derive(Clone, Copy)]
struct ObjectPixel {
    color: u8,
    palette: u8,
    sprite_prio: u8,
    bg_prio: bool,
}

#[derive(Clone, Copy)]
struct BackgroundPixel {
    color: u8,
    palette: u8,
}

type RenderedPixel = u8;

// Todo: Implement ppu vram blocking
// Todo: Implement window rendering penalty
// Todo: More complex behavior for cgb palette access
// Todo: Original gameboy compatibility
pub struct PPU {
    mode: Rc<RefCell<dyn PPUMode>>,
    memory: Rc<RefCell<MemManager>>,
    current_frame: Vec<RenderedPixel>,
    completed_frame: Vec<RenderedPixel>,
    mode_dots_passed: u32,
    objects_on_scanline: Vec<u16>,
    object_pixel_queue: VecDeque<ObjectPixel>,
    background_pixel_queue: VecDeque<BackgroundPixel>,
}

impl PPU {
    pub fn new(memory: Rc<RefCell<MemManager>>) -> Self {
        let mut ppu = PPU::new_test(memory);
        // Needed because emulator starts at pc = 0x0100 instead of actual hardware that starts at pc = 0x0000
        ppu.update(DOTS_PER_SCANLINE * 147 + 180);
        ppu
    }

    fn new_test(memory: Rc<RefCell<MemManager>>) -> Self {
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

    pub fn get_frame(&self) -> Vec<u8> {
        self.completed_frame.clone()
    }

    fn current_scanline(&self) -> u8 {
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
        let coincidence_enabled = stat_value & 0b01000000 != 0;
        if coincidence_enabled && self.current_scanline() == self.memory.borrow().read(LYC_ADDRESS)
        {
            // Set STAT flag
            self.memory
                .borrow_mut()
                .write(STAT_ADDRESS, stat_value | 0b00000100);
            // Set IF flag
            let if_value = self.memory.borrow().read(IF_ADDRESS);
            self.memory
                .borrow_mut()
                .write(IF_ADDRESS, if_value | 0b00000010);
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

struct HBlank {
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

struct VBlank;
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
            let object_end = ppu.memory.borrow().read(address);
            if object_end == 0 {
                continue;
            }
            let object_size = if large_objects { 16 } else { 8 };
            let object_start = if object_end < object_size {
                0
            } else {
                object_end - object_size
            };

            let object_pixel_range = object_start..object_end;
            if object_pixel_range.contains(&ppu.current_scanline()) {
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
        let new_mode = Rc::new(RefCell::new(Draw {
            fetcher: Fetcher::new(),
            screen_x: 0,
        }));
        // Perform one fetch early for timing purposes
        for _ in 0..6 {
            new_mode.borrow_mut().fetcher.tick(ppu);
        }
        ppu.set_mode(new_mode.clone());
    }

    fn get_mode_number(&self) -> u8 {
        2
    }
}

enum FetcherStage {
    GetTile,
    DataLow,
    DataHigh,
    Push,
}

struct Fetcher {
    tilemap_col: u8,
    sprites_to_fetch: VecDeque<u16>,
    dots_since_fetch_start: u32,
    stage: FetcherStage,
    tile_index: Option<u8>,
    tile_address: Option<u16>, // Can delete later
    bg_tile_attributes: Option<u8>,
    tile_data_low: Option<u8>,
    tile_data_high: Option<u8>,
    current_sprite: Option<u16>,
    is_fetching_object: bool,
    was_fetching_bg: bool,
}

impl Fetcher {
    fn new() -> Self {
        Self {
            tilemap_col: 0,
            sprites_to_fetch: VecDeque::new(),
            dots_since_fetch_start: 0,
            stage: GetTile,
            tile_index: None,
            tile_address: None,
            bg_tile_attributes: None,
            tile_data_low: None,
            tile_data_high: None,
            current_sprite: None,
            is_fetching_object: false,
            was_fetching_bg: true,
        }
    }

    fn tick(&mut self, ppu: &mut PPU) {
        self.dots_since_fetch_start += 1;
        let current_dots = if self.is_fetching_object && self.was_fetching_bg {
            // Sprite fetches overlap background fetches by one dot.
            // This effectively means sprite fetches take one dot less if they follow
            // background fetches.
            self.dots_since_fetch_start + 1
        } else {
            self.dots_since_fetch_start
        };

        match self.stage {
            GetTile => {
                if current_dots == 2 {
                    match self.is_fetching_object {
                        true => {
                            self.current_sprite = self.sprites_to_fetch.pop_front();
                            let sprite = self.current_sprite.unwrap();
                            self.tile_index =
                                Some(ppu.memory.borrow().read(self.current_sprite.unwrap() + 2))
                        }
                        false => {
                            self.tile_address = Some(self.get_tile_address(ppu));
                            self.tile_index = Some(self.get_tile_index(ppu));
                            self.bg_tile_attributes = Some(self.get_bg_tile_attributes(ppu));
                            self.tilemap_col += 1;
                        }
                    };
                    self.stage = DataLow;
                }
            }
            DataLow => {
                if current_dots == 4 {
                    self.tile_data_low = Some(self.get_tile_data(
                        ppu,
                        self.tile_index.unwrap(),
                        false,
                        self.is_fetching_object,
                    ));
                    self.stage = DataHigh;
                }
            }
            DataHigh => {
                if current_dots == 6 {
                    self.tile_data_high = Some(self.get_tile_data(
                        ppu,
                        self.tile_index.unwrap(),
                        true,
                        self.is_fetching_object,
                    ));
                    self.stage = Push;
                    // Check if ready to push immediately
                    self.tick(ppu);
                }
            }
            Push => {
                let pixels = self.get_pixels_from_tile_data(
                    self.tile_data_low.unwrap(),
                    self.tile_data_high.unwrap(),
                );
                match self.is_fetching_object {
                    true => {
                        ppu.object_pixel_queue.clear();
                        self.push_object_pixels(ppu, pixels);
                        self.start_new_fetch(ppu);
                    }
                    false => {
                        if ppu.background_pixel_queue.len() <= 8 {
                            let attrs = self.bg_tile_attributes.unwrap();
                            self.push_background_pixels(ppu, pixels, attrs);
                            self.start_new_fetch(ppu);
                        } else if self.is_sprite_queued() {
                            self.start_new_fetch(ppu);
                        }
                    }
                }
            }
        }
    }

    fn start_new_fetch(&mut self, ppu: &PPU) {
        self.was_fetching_bg = !self.is_fetching_object;
        self.is_fetching_object = self.is_sprite_queued() && ppu.background_pixel_queue.len() >= 8;
        self.stage = GetTile;
        self.tile_index = None;
        self.tile_address = None;
        self.bg_tile_attributes = None;
        self.tile_data_low = None;
        self.tile_data_high = None;
        self.current_sprite = None;
        self.dots_since_fetch_start = 0;
    }

    fn get_tile_address(&mut self, ppu: &PPU) -> u16 {
        let mem = ppu.memory.borrow();
        let lcdc = mem.read(LCDC_ADDRESS);

        let wx = mem.read(WX_ADDRESS).wrapping_sub(7);
        let wy = mem.read(WY_ADDRESS);
        let current_scanline = ppu.current_scanline();
        let is_window_tile = current_scanline >= wy / 8 && (self.tilemap_col >= wx / 8);
        let window_enabled = lcdc & 0b00100000 != 0;
        let uses_window = window_enabled && is_window_tile;

        let scx = mem.read(SCX_ADDRESS);
        let scy = mem.read(SCY_ADDRESS);
        let screen_offset_x = if uses_window { 0 } else { scx / 8 };
        let screen_offset_y = if uses_window { 0 } else { scy };

        let tilemap_row_width: u16 = 32;
        let tilemap_x = ((screen_offset_x + self.tilemap_col) & 0x1F) as u16;
        let tilemap_y = (current_scanline.wrapping_add(screen_offset_y) / 8) as u16;
        let tile_position = tilemap_y * tilemap_row_width + tilemap_x;

        let bg_switch_tilemap = lcdc & 0b00001000 != 0;
        let wd_switch_tilemap = lcdc & 0b01000000 != 0;
        let tilemap_start: u16 =
            if (uses_window && wd_switch_tilemap) || (!uses_window && bg_switch_tilemap) {
                0x9C00
            } else {
                0x9800
            };

        tile_position + tilemap_start
    }

    fn get_tile_index(&mut self, ppu: &PPU) -> u8 {
        let tile_address = self.get_tile_address(ppu);

        let initial = ppu.memory.borrow().read(VBK_ADDRESS);
        ppu.memory.borrow_mut().write(VBK_ADDRESS, 0x00);
        let data = ppu.memory.borrow().read(tile_address);
        ppu.memory.borrow_mut().write(VBK_ADDRESS, initial);
        data
    }

    fn get_bg_tile_attributes(&mut self, ppu: &PPU) -> u8 {
        let tile_address = self.get_tile_address(ppu);

        let initial = ppu.memory.borrow().read(VBK_ADDRESS);
        ppu.memory.borrow_mut().write(VBK_ADDRESS, 0x01);
        let data = ppu.memory.borrow().read(tile_address);
        ppu.memory.borrow_mut().write(VBK_ADDRESS, initial);
        data
    }

    fn get_tile_data(&mut self, ppu: &PPU, index: u8, is_high_byte: bool, is_object: bool) -> u8 {
        let lcdc = ppu.memory.borrow().read(LCDC_ADDRESS);
        let signed_addressing = !is_object && lcdc & 0b00010000 == 0;
        let base_address = if signed_addressing {
            let signed_index: i32 = if index > 127 {
                -(128 - (index as i32 - 128))
            } else {
                index as i32
            };
            (0x9000 + signed_index * 16) as u16
        } else {
            0x8000 + (index as u16) * 16
        };

        let attrs = if is_object {
            let sprite_address = self.current_sprite.unwrap();
            ppu.memory.borrow().read(sprite_address + 3)
        } else {
            self.bg_tile_attributes.unwrap()
        };

        let using_large_objects = lcdc & 0b00000100 != 0;
        let height = if is_object && using_large_objects {
            16
        } else {
            8
        };

        let is_flipped_vertically = attrs & 0b01000000 != 0;
        let row_offset = if is_flipped_vertically {
            ((height - 1) - (ppu.current_scanline() % height)) * 2
        } else {
            (ppu.current_scanline() % height) * 2
        };
        let high_byte_offset = if is_high_byte { 1 } else { 0 };
        let uses_vram_bank_one = attrs & 0b00001000 != 0;
        if uses_vram_bank_one {
            let initial = ppu.memory.borrow().read(VBK_ADDRESS);
            ppu.memory.borrow_mut().write(VBK_ADDRESS, 0x01);
            let data = ppu
                .memory
                .borrow()
                .read(base_address + high_byte_offset + row_offset as u16);
            ppu.memory.borrow_mut().write(VBK_ADDRESS, initial);
            data
        } else {
            let initial = ppu.memory.borrow().read(VBK_ADDRESS);
            ppu.memory.borrow_mut().write(VBK_ADDRESS, 0x00);
            let data = ppu
                .memory
                .borrow()
                .read(base_address + high_byte_offset + row_offset as u16);
            ppu.memory.borrow_mut().write(VBK_ADDRESS, initial);
            data
        }
    }

    fn get_pixels_from_tile_data(&self, tile_data_low: u8, tile_data_high: u8) -> VecDeque<u8> {
        let mut pixels = VecDeque::new();
        for i in 0..8 {
            let mask = 0b00000001 << i;
            let low_pixel_bit = (mask & tile_data_low) >> i;
            let high_pixel_bit = (mask & tile_data_high) >> i;
            pixels.push_back(low_pixel_bit + (high_pixel_bit << 1));
        }
        pixels
    }

    fn push_background_pixels(&self, ppu: &mut PPU, mut pixels: VecDeque<u8>, attrs: u8) {
        let is_flipped_horizontal = attrs & 0b00100000 != 0;
        let mut pop_pixel = || {
            if is_flipped_horizontal {
                pixels.pop_front()
            } else {
                pixels.pop_back()
            }
        };

        let palette = attrs & 0b00000111;
        for _ in 0..8 {
            let color = pop_pixel().unwrap();
            ppu.background_pixel_queue
                .push_back(BackgroundPixel { color, palette });
        }
    }

    fn push_object_pixels(&self, ppu: &mut PPU, mut pixels: VecDeque<u8>) {
        let sprite_address = self.current_sprite.unwrap();
        let attrs = ppu.memory.borrow().read(sprite_address + 3);

        let is_flipped_horizontal = attrs & 0b00010000 != 0;
        let mut pop_pixel = || {
            if is_flipped_horizontal {
                pixels.pop_front()
            } else {
                pixels.pop_back()
            }
        };

        let palette = attrs & 0b00000111;
        let bg_prio = if attrs & 0b10000000 != 0 { true } else { false };
        let sprite_prio = ((sprite_address - 0xFE00) / 4) as u8;
        for _ in 0..8 {
            let color = pop_pixel().unwrap();
            ppu.object_pixel_queue.push_back(ObjectPixel {
                color,
                palette,
                sprite_prio,
                bg_prio,
            })
        }
    }

    fn is_sprite_queued(&self) -> bool {
        !self.sprites_to_fetch.is_empty()
    }

    fn schedule_sprite_fetch(&mut self, object_address: u16) {
        if !self.sprites_to_fetch.contains(&object_address) {
            self.sprites_to_fetch.push_back(object_address);
        }
    }
}

struct Draw {
    fetcher: Fetcher,
    screen_x: u8,
}

impl Draw {
    fn tick(&mut self, ppu: &mut PPU) -> bool {
        ppu.mode_dots_passed += 1;

        if ppu.background_pixel_queue.len() > 8 && !self.fetcher.is_sprite_queued() {
            // Throw away the pixels that are cut off by screen scroll
            if ppu.mode_dots_passed <= (ppu.memory.borrow().read(SCX_ADDRESS) % 8) as u32 {
                let _ = ppu.background_pixel_queue.pop_front();
                if !ppu.background_pixel_queue.is_empty() {
                    let _ = ppu.object_pixel_queue.pop_front();
                }
            } else if self.screen_x < 160 {
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
                if object_start
                    == self
                        .screen_x
                        .wrapping_add(ppu.memory.borrow().read(SCX_ADDRESS))
                {
                    self.fetcher.schedule_sprite_fetch(*object_address);
                }
            }
            self.screen_x += 1;

            if self.screen_x >= 166 {
                return false;
            }
        }
        self.fetcher.tick(ppu);
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
        ppu.set_mode(Rc::new(RefCell::new(Draw {
            screen_x: 0,
            fetcher: Fetcher::new(),
        })));
        ppu.update(166);
        assert_eq!(ppu.mode.borrow().get_mode_number(), 3);
    }

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

    #[test]
    fn screen_wraps_around_vertical_scy_is_greater_than_screen_height() {
        let ppu = get_test_ppu();
        ppu.memory.borrow_mut().write(SCY_ADDRESS, 0x98);
        ppu.memory.borrow_mut().write(0x9800, 0x30);
        ppu.memory.borrow_mut().write(LY_ADDRESS, 104);
        let mut fetcher = Fetcher::new();
        assert_eq!(fetcher.get_tile_index(&ppu), 0x30);
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
        fetcher.tilemap_col += 1;
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
        let mut ppu = get_test_ppu();
        ppu.memory.borrow_mut().write(0x8000, 0x11);
        let mut fetcher = Fetcher::new();
        fetcher.bg_tile_attributes = Some(0);
        let result = fetcher.get_tile_data(&mut ppu, 0, false, false);
        assert_eq!(result, 0x11);
    }

    #[test]
    fn gets_tile_data_first_row_second_byte() {
        let mut ppu = get_test_ppu();
        ppu.memory.borrow_mut().write(0x8001, 0x11);
        let mut fetcher = Fetcher::new();
        fetcher.bg_tile_attributes = Some(0);
        let result = fetcher.get_tile_data(&mut ppu, 0, true, false);
        assert_eq!(result, 0x11);
    }

    #[test]
    fn gets_tile_data_second_row_first_byte() {
        let mut ppu = get_test_ppu();
        ppu.memory.borrow_mut().write(0x8002, 0x11);
        ppu.memory.borrow_mut().write(LY_ADDRESS, 0x01);
        let mut fetcher = Fetcher::new();
        fetcher.bg_tile_attributes = Some(0);
        let result = fetcher.get_tile_data(&mut ppu, 0, false, false);
        assert_eq!(result, 0x11);
    }

    #[test]
    fn gets_tile_data_seventh_row_second_byte() {
        let mut ppu = get_test_ppu();
        ppu.memory.borrow_mut().write(0x800F, 0x11);
        ppu.memory.borrow_mut().write(LY_ADDRESS, 0x07);
        let mut fetcher = Fetcher::new();
        fetcher.bg_tile_attributes = Some(0);
        let result = fetcher.get_tile_data(&mut ppu, 0, true, false);
        assert_eq!(result, 0x11);
    }

    #[test]
    fn gets_tile_data_seventh_row_second_byte_vertically_flipped() {
        let mut ppu = get_test_ppu();
        ppu.memory.borrow_mut().write(0x8001, 0x11);
        ppu.memory.borrow_mut().write(LY_ADDRESS, 0x07);
        let mut fetcher = Fetcher::new();
        fetcher.bg_tile_attributes = Some(0b01000000);
        let result = fetcher.get_tile_data(&mut ppu, 0, true, false);
        assert_eq!(result, 0x11);
    }

    #[test]
    fn gets_tile_data_from_second_vram_bank() {
        let mut ppu = get_test_ppu();
        let mut fetcher = Fetcher::new();
        fetcher.bg_tile_attributes = Some(0b00001000);
        ppu.memory.borrow_mut().write(LY_ADDRESS, 0x01);
        ppu.memory.borrow_mut().write(VBK_ADDRESS, 1);
        ppu.memory.borrow_mut().write(0x8002, 0x11);
        ppu.memory.borrow_mut().write(VBK_ADDRESS, 0);
        let result = fetcher.get_tile_data(&mut ppu, 0, false, false);
        assert_eq!(result, 0x11);
    }

    #[test]
    fn gets_tile_data_from_second_block_of_tiles_unsigned_addressing() {
        let mut ppu = get_test_ppu();
        let mut fetcher = Fetcher::new();
        fetcher.bg_tile_attributes = Some(0);
        ppu.memory.borrow_mut().write(0x8300, 0x11);
        let result = fetcher.get_tile_data(&mut ppu, 0x30, false, false);
        assert_eq!(result, 0x11);
    }

    #[test]
    fn get_pixels_from_tile_data_correct_ordering() {
        let fetcher = Fetcher::new();
        let pixels = fetcher.get_pixels_from_tile_data(0xFF, 0x00);
        for pix in pixels {
            assert_eq!(pix, 1);
        }
    }

    #[test]
    fn get_pixels_from_tile_data_every_value_in_row() {
        let fetcher = Fetcher::new();
        let pixels = fetcher.get_pixels_from_tile_data(0x7C, 0x56);
        assert_eq!(pixels, [0, 2, 3, 1, 3, 1, 3, 0]);
    }

    #[test]
    fn get_bg_tile_attributes_gets_correct_value() {
        let ppu = get_test_ppu();
        let mut fetcher = Fetcher::new();
        ppu.memory.borrow_mut().write(0x9805, 0xA7);
        ppu.memory.borrow_mut().write(VBK_ADDRESS, 0x01);
        ppu.memory.borrow_mut().write(0x9805, 0xA8);
        ppu.memory.borrow_mut().write(VBK_ADDRESS, 0x00);
        fetcher.tilemap_col = 5;
        let index = fetcher.get_tile_index(&ppu);
        let attrs = fetcher.get_bg_tile_attributes(&ppu);
        assert_eq!((index, attrs), (0xA7, 0xA8));
    }

    #[test]
    fn pushing_background_pixels_maintains_value() {
        let ppu = &mut get_test_ppu();
        let fetcher = Fetcher::new();
        let pixels = fetcher.get_pixels_from_tile_data(0x7C, 0x56);
        let attrs = 0;
        fetcher.push_background_pixels(ppu, pixels, attrs);
        for (pix, known_color) in ppu
            .background_pixel_queue
            .iter()
            .zip([0, 3, 1, 3, 1, 3, 2, 0])
        {
            assert_eq!(pix.color, known_color);
        }
    }

    #[test]
    fn pushing_background_pixels_maintains_value_horizontally_flipped() {
        let ppu = &mut get_test_ppu();
        let fetcher = Fetcher::new();
        let pixels = fetcher.get_pixels_from_tile_data(0x7C, 0x56);
        let attrs = 0b00100000;
        fetcher.push_background_pixels(ppu, pixels, attrs);
        for (pix, known_color) in ppu
            .background_pixel_queue
            .iter()
            .zip([0, 2, 3, 1, 3, 1, 3, 0])
        {
            assert_eq!(pix.color, known_color);
        }
    }

    #[test]
    fn pushing_background_pixels_go_after_current_pixels_in_queue() {
        let mut ppu = get_test_ppu();
        let fetcher = Fetcher::new();
        let attrs = 0;
        let first_pixels = VecDeque::from([3, 3, 3, 3, 3, 3, 3, 3]);
        fetcher.push_background_pixels(&mut ppu, first_pixels, attrs);
        let second_pixels = VecDeque::from([0, 0, 0, 0, 0, 0, 0, 0]);
        fetcher.push_background_pixels(&mut ppu, second_pixels, attrs);
        assert_eq!(ppu.background_pixel_queue[15].color, 0);
        assert_eq!(ppu.background_pixel_queue[8].color, 0);
    }

    #[test]
    fn pushing_background_pixels_go_after_current_pixels_in_queue_flipped_horizontal() {
        let mut ppu = get_test_ppu();
        let fetcher = Fetcher::new();
        let attrs = 0b00100000;
        let first_pixels = VecDeque::from([3, 3, 3, 3, 3, 3, 3, 3]);
        fetcher.push_background_pixels(&mut ppu, first_pixels, attrs);
        let second_pixels = VecDeque::from([0, 0, 0, 0, 0, 0, 0, 0]);
        fetcher.push_background_pixels(&mut ppu, second_pixels, attrs);
        assert_eq!(ppu.background_pixel_queue[15].color, 0);
        assert_eq!(ppu.background_pixel_queue[8].color, 0);
    }

    #[test]
    fn pushing_object_pixels_maintains_value() {
        let ppu = &mut get_test_ppu();
        let mut fetcher = Fetcher::new();
        let pixels = fetcher.get_pixels_from_tile_data(0x7C, 0x56);
        fetcher.current_sprite = Some(0xFE00);
        fetcher.push_object_pixels(ppu, pixels);
        for (pix, known_color) in ppu
            .background_pixel_queue
            .iter()
            .zip([0, 3, 1, 3, 1, 3, 2, 0])
        {
            assert_eq!(pix.color, known_color);
        }
    }

    #[test]
    fn pushing_object_pixels_maintains_value_horizontally_flipped() {
        let ppu = &mut get_test_ppu();
        let mut fetcher = Fetcher::new();
        let pixels = fetcher.get_pixels_from_tile_data(0x7C, 0x56);
        fetcher.current_sprite = Some(0xFE00);
        fetcher.push_object_pixels(ppu, pixels);
        for (pix, known_color) in ppu
            .background_pixel_queue
            .iter()
            .zip([0, 2, 3, 1, 3, 1, 3, 0])
        {
            assert_eq!(pix.color, known_color);
        }
    }

    #[test]
    fn same_object_is_not_queued_more_than_once() {
        let mut ppu = get_test_ppu();
        set_obj_y_pos(&mut ppu, 0, 16);
        ppu.memory.borrow_mut().write(0xFE01, 0x08);
        ppu.update(80); // Complete oam scan and transition to draw
        assert_eq!(ppu.objects_on_scanline[0], 0xFE00);
        let mut fetcher = Fetcher::new();
        let mut draw = Draw {
            screen_x: 0,
            fetcher,
        };
        for _ in 0..12 {
            draw.tick(&mut ppu);
        }
        assert_eq!(draw.fetcher.sprites_to_fetch[0], 0xFE00);
        for i in 1..draw.fetcher.sprites_to_fetch.len() {
            assert_ne!(draw.fetcher.sprites_to_fetch[i], 0xFE00);
        }
    }

    #[test]
    fn gets_correct_value_for_palette() {
        let mut ppu = get_test_ppu();
        let draw = Draw {
            screen_x: 0,
            fetcher: Fetcher::new(),
        };
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
        let mut fetcher = Fetcher::new();
        fetcher.current_sprite = Some(0);
        let res = fetcher.get_tile_data(&ppu, 0, false, true);
        assert_eq!(res, 16);
    }
}
