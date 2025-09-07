use std::collections::VecDeque;

use crate::memory::Memory;

use crate::ppu::{BackgroundPixel, ObjectPixel, PPU};

use crate::fetcher::FetcherStage::{DataHigh, DataLow, GetTile, Push};

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

enum FetcherStage {
    GetTile,
    DataLow,
    DataHigh,
    Push,
}

pub struct BackgroundFetcher {
    tilemap_col: u8,
    current_dots: u32,
    stage: FetcherStage,
    tile_index: Option<u8>,
    tile_attrs: Option<u8>,
    tile_data_low: Option<u8>,
    tile_data_high: Option<u8>,
    in_window: bool,
}

impl BackgroundFetcher {
    pub(crate) fn new() -> Self {
        Self {
            tilemap_col: 0,
            current_dots: 0,
            stage: GetTile,
            tile_index: None,
            tile_attrs: None,
            tile_data_low: None,
            tile_data_high: None,
            in_window: false,
        }
    }

    pub(crate) fn tick(&mut self, ppu: &mut PPU) {
        self.current_dots += 1;

        match self.stage {
            GetTile => {
                if self.current_dots == 2 {
                    self.tile_index = Some(self.get_tile_index(ppu));
                    self.tile_attrs = Some(self.get_bg_tile_attributes(ppu));
                    self.tilemap_col += 1;
                    self.stage = DataLow;
                }
            }
            DataLow => {
                if self.current_dots == 4 {
                    self.tile_data_low =
                        Some(self.get_tile_data(ppu, self.tile_index.unwrap(), false));
                    self.stage = DataHigh;
                }
            }
            DataHigh => {
                if self.current_dots == 6 {
                    self.tile_data_high =
                        Some(self.get_tile_data(ppu, self.tile_index.unwrap(), true));
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
                if ppu.background_pixel_queue.len() <= 8 {
                    let attrs = self.tile_attrs.unwrap();
                    self.push_background_pixels(ppu, pixels, attrs);
                    self.start_new_fetch();
                }
            }
        }
    }

    pub(crate) fn start_new_fetch(&mut self) {
        self.stage = GetTile;
        self.tile_index = None;
        self.tile_data_low = None;
        self.tile_data_high = None;
        self.current_dots = 0;
    }

    pub(crate) fn get_tile_address(&mut self, ppu: &PPU) -> u16 {
        let mem = ppu.memory.borrow();
        let lcdc = mem.read(LCDC_ADDRESS);

        let wx = mem.read(WX_ADDRESS).wrapping_sub(7);
        let wy = mem.read(WY_ADDRESS);
        let current_scanline = ppu.get_current_scanline();

        let is_window_tile = current_scanline >= wy && ppu.screen_x >= wx;
        let window_enabled = lcdc & 0b00100000 != 0;
        let window_active = window_enabled && is_window_tile;

        let scx = mem.read(SCX_ADDRESS);
        let scy = mem.read(SCY_ADDRESS);

        return if window_active {
            if !self.in_window {
                self.tilemap_col = 0;
                self.in_window = true;
            }
            let window_x = self.tilemap_col;
            let window_y = (current_scanline - wy) / 8;

            let tilemap_row_width: u16 = 32;
            let tilemap_x = window_x & 0x1F;
            let tilemap_y = window_y & 0x1F;

            let window_tilemap = if lcdc & 0b01000000 != 0 {
                0x9C00
            } else {
                0x9800
            };

            let tile_position = tilemap_y as u16 * tilemap_row_width + tilemap_x as u16;

            window_tilemap + tile_position
        } else {
            let screen_offset_x = scx / 8;
            let screen_offset_y = scy;

            let tilemap_row_width: u16 = 32;
            let tilemap_x = ((screen_offset_x + self.tilemap_col) & 0x1F) as u16;
            let tilemap_y = (current_scanline.wrapping_add(screen_offset_y) / 8) as u16;

            let bg_tilemap: u16 = if lcdc & 0b00001000 != 0 {
                0x9C00
            } else {
                0x9800
            };

            let tile_position = tilemap_y * tilemap_row_width + tilemap_x;

            bg_tilemap + tile_position
        };
    }

    pub(crate) fn get_tile_index(&mut self, ppu: &PPU) -> u8 {
        let tile_address = self.get_tile_address(ppu);

        let initial = ppu.memory.borrow().read(VBK_ADDRESS);
        ppu.memory.borrow_mut().write(VBK_ADDRESS, 0x00);
        let data = ppu.memory.borrow().read(tile_address);
        ppu.memory.borrow_mut().write(VBK_ADDRESS, initial);
        data
    }

    pub(crate) fn get_bg_tile_attributes(&mut self, ppu: &PPU) -> u8 {
        let tile_address = self.get_tile_address(ppu);

        let initial = ppu.memory.borrow().read(VBK_ADDRESS);
        ppu.memory.borrow_mut().write(VBK_ADDRESS, 0x01);
        let data = ppu.memory.borrow().read(tile_address);
        ppu.memory.borrow_mut().write(VBK_ADDRESS, initial);
        data
    }

    pub(crate) fn get_tile_data(&mut self, ppu: &PPU, index: u8, is_high_byte: bool) -> u8 {
        let lcdc = ppu.memory.borrow().read(LCDC_ADDRESS);
        let signed_addressing = lcdc & 0b00010000 == 0;
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

        let attrs = self.tile_attrs.unwrap();
        let height = 8;

        let is_flipped_vertically = attrs & 0b01000000 != 0;
        let row_offset = if is_flipped_vertically {
            ((height - 1) - (ppu.get_current_scanline() % height)) * 2
        } else {
            (ppu.get_current_scanline() % height) * 2
        };
        let high_byte_offset = if is_high_byte { 1 } else { 0 };

        let initial = ppu.memory.borrow().read(VBK_ADDRESS);

        let uses_vram_bank_one = attrs & 0b00001000 != 0;
        if uses_vram_bank_one {
            ppu.memory.borrow_mut().write(VBK_ADDRESS, 0x01);
        } else {
            ppu.memory.borrow_mut().write(VBK_ADDRESS, 0x00);
        }
        let data = ppu
            .memory
            .borrow()
            .read(base_address + high_byte_offset + row_offset as u16);
        ppu.memory.borrow_mut().write(VBK_ADDRESS, initial);
        data
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
}

pub(crate) struct SpriteFetcher {
    stage: FetcherStage,
    current_dots: u8,
    current_sprite: Option<u16>,
    tile_index: Option<u8>,
    tile_data_low: Option<u8>,
    tile_data_high: Option<u8>,
}

impl SpriteFetcher {
    pub fn new() -> Self {
        SpriteFetcher {
            stage: GetTile,
            current_dots: 0,
            current_sprite: None,
            tile_index: None,
            tile_data_low: None,
            tile_data_high: None,
        }
    }

    pub(crate) fn start_fetch(&mut self, sprite_address: u16) {
        self.reset();
        self.current_sprite = Some(sprite_address);
    }

    pub(crate) fn tick(&mut self, ppu: &mut PPU) {
        self.current_dots += 1;
        match self.stage {
            GetTile => {
                if self.current_dots == 2 {
                    let sprite = self.current_sprite.unwrap();
                    self.tile_index = Some(ppu.memory.borrow().read(sprite + 2));
                    self.stage = DataLow;
                }
            }
            DataLow => {
                if self.current_dots == 4 {
                    self.tile_data_low =
                        Some(self.get_tile_data(ppu, self.tile_index.unwrap(), false));
                    self.stage = DataHigh;
                }
            }
            DataHigh => {
                if self.current_dots == 6 {
                    self.tile_data_high =
                        Some(self.get_tile_data(ppu, self.tile_index.unwrap(), true));
                    self.stage = Push;
                }
            }
            Push => {
                let pixels = self.get_pixels_from_tile_data(
                    self.tile_data_low.unwrap(),
                    self.tile_data_high.unwrap(),
                );
                ppu.object_pixel_queue.clear();
                self.push_object_pixels(ppu, pixels);
                self.reset();
            }
        }
    }

    pub(crate) fn reset(&mut self) {
        self.stage = GetTile;
        self.tile_index = None;
        self.tile_data_low = None;
        self.tile_data_high = None;
        self.current_sprite = None;
        self.current_dots = 0;
    }

    pub(crate) fn get_tile_data(&mut self, ppu: &PPU, index: u8, is_high_byte: bool) -> u8 {
        let lcdc = ppu.memory.borrow().read(LCDC_ADDRESS);
        let base_address = 0x8000 + (index as u16) * 16;

        let attrs = {
            let sprite_address = self.current_sprite.unwrap();
            ppu.memory.borrow().read(sprite_address + 3)
        };

        let using_large_objects = lcdc & 0b00000100 != 0;
        let height = if using_large_objects { 16 } else { 8 };

        let object_y = ppu.memory.borrow().read(self.current_sprite.unwrap());
        let sprite_screen_y = object_y.wrapping_sub(16);
        let row_in_sprite = ppu.get_current_scanline().wrapping_sub(sprite_screen_y);

        let is_flipped_vertically = attrs & 0b01000000 != 0;
        let row_offset = if is_flipped_vertically {
            (height - 1) - row_in_sprite
        } else {
            row_in_sprite
        } * 2;
        let high_byte_offset = if is_high_byte { 1 } else { 0 };

        let initial = ppu.memory.borrow().read(VBK_ADDRESS);

        let uses_vram_bank_one = attrs & 0b00001000 != 0;
        if uses_vram_bank_one {
            ppu.memory.borrow_mut().write(VBK_ADDRESS, 0x01);
        } else {
            ppu.memory.borrow_mut().write(VBK_ADDRESS, 0x00);
        }
        let data = ppu
            .memory
            .borrow()
            .read(base_address + high_byte_offset + row_offset as u16);
        ppu.memory.borrow_mut().write(VBK_ADDRESS, initial);
        data
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

    fn push_object_pixels(&self, ppu: &mut PPU, mut pixels: VecDeque<u8>) {
        let sprite_address = self.current_sprite.unwrap();
        let attrs = ppu.memory.borrow().read(sprite_address + 3);

        let is_flipped_horizontal = attrs & 0b00100000 != 0;
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

    pub(crate) fn has_sprite_queued(&self) -> bool {
        self.current_sprite.is_some()
    }
}

#[cfg(test)]
mod tests {
    use std::{cell::RefCell, rc::Rc};

    use crate::mem_manager::MemManager;

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
    fn fetcher_gets_index_of_first_tile() {
        let ppu = get_test_ppu();
        ppu.memory.borrow_mut().write(0x9800, 0xAA);
        let mut fetcher = BackgroundFetcher::new();
        assert_eq!(fetcher.get_tile_index(&ppu), 0xAA);
    }

    #[test]
    fn fetcher_gets_index_for_second_tile() {
        let ppu = get_test_ppu();
        ppu.memory.borrow_mut().write(0x9801, 0xBB);
        let mut fetcher = BackgroundFetcher::new();
        fetcher.tilemap_col += 1;
        assert_eq!(fetcher.get_tile_index(&ppu), 0xBB);
    }

    #[test]
    fn fetcher_gets_index_for_second_row() {
        let ppu = get_test_ppu();
        ppu.memory.borrow_mut().write(LY_ADDRESS, 0x08);
        ppu.memory.borrow_mut().write(0x9820, 0xCC);
        let mut fetcher = BackgroundFetcher::new();
        assert_eq!(fetcher.get_tile_index(&ppu), 0xCC);
    }

    #[test]
    fn fetcher_gets_index_for_final_row() {
        let ppu = get_test_ppu();
        ppu.memory.borrow_mut().write(LY_ADDRESS, 143);
        ppu.memory.borrow_mut().write(0x9800 + 32 * 17, 0xCC);
        let mut fetcher = BackgroundFetcher::new();
        assert_eq!(fetcher.get_tile_index(&ppu), 0xCC);
    }

    #[test]
    fn fetcher_gets_index_for_final_column() {
        let ppu = get_test_ppu();
        ppu.memory.borrow_mut().write(WX_ADDRESS, 255);
        ppu.memory.borrow_mut().write(0x9807, 0xCC);
        let mut fetcher = BackgroundFetcher::new();
        fetcher.tilemap_col = 7;
        assert_eq!(fetcher.get_tile_index(&ppu), 0xCC);
    }

    #[test]
    fn background_index_fetching_works_with_second_tilemap() {
        let ppu = get_test_ppu();
        ppu.memory.borrow_mut().write(0x9C00, 0xAA);
        ppu.memory.borrow_mut().write(LCDC_ADDRESS, 0b10011001);
        let mut fetcher = BackgroundFetcher::new();
        assert_eq!(fetcher.get_tile_index(&ppu), 0xAA);
    }

    #[test]
    fn fetcher_gets_correct_index_with_screen_scrolled() {
        let ppu = get_test_ppu();
        ppu.memory.borrow_mut().write(SCX_ADDRESS, 95);
        ppu.memory.borrow_mut().write(SCY_ADDRESS, 111);
        ppu.memory.borrow_mut().write(0x99AB, 0xAA);
        ppu.memory.borrow_mut().write(LCDC_ADDRESS, 0b10010001);
        let mut fetcher = BackgroundFetcher::new();
        assert_eq!(fetcher.get_tile_index(&ppu), 0xAA);
    }

    #[test]
    fn screen_wraps_around_horizontal() {
        let ppu = get_test_ppu();
        ppu.memory.borrow_mut().write(WY_ADDRESS, 255);
        ppu.memory.borrow_mut().write(0x9800, 0xDD);
        ppu.memory.borrow_mut().write(SCX_ADDRESS, 80);
        let mut fetcher = BackgroundFetcher::new();
        fetcher.tilemap_col = 22;
        assert_eq!(fetcher.get_tile_index(&ppu), 0xDD);
    }

    #[test]
    fn fetcher_gets_window_index_first_row_first_column() {
        let ppu = get_test_ppu();
        ppu.memory.borrow_mut().write(WX_ADDRESS, 7);
        ppu.memory.borrow_mut().write(WY_ADDRESS, 0);
        ppu.memory.borrow_mut().write(0x9800, 0xAA);
        ppu.memory.borrow_mut().write(LCDC_ADDRESS, 0b10110001);
        let mut fetcher = BackgroundFetcher::new();
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
        let mut fetcher = BackgroundFetcher::new();
        assert_eq!(fetcher.get_tile_index(&ppu), 0xAA);
    }

    #[test]
    fn fetches_two_consecutive_window_indices() {
        let ppu = get_test_ppu();
        ppu.memory.borrow_mut().write(WX_ADDRESS, 7);
        ppu.memory.borrow_mut().write(WY_ADDRESS, 0);
        ppu.memory.borrow_mut().write(0x9801, 0xBA);
        ppu.memory.borrow_mut().write(LCDC_ADDRESS, 0b10110001);
        let mut fetcher = BackgroundFetcher::new();
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
        let mut fetcher = BackgroundFetcher::new();
        assert_eq!(fetcher.get_tile_index(&ppu), 0xAA);
    }

    #[test]
    fn gets_tile_data_first_row_first_byte() {
        let mut ppu = get_test_ppu();
        ppu.memory.borrow_mut().write(0x8000, 0x11);
        let mut fetcher = BackgroundFetcher::new();
        fetcher.tile_attrs = Some(0);
        let result = fetcher.get_tile_data(&mut ppu, 0, false);
        assert_eq!(result, 0x11);
    }

    #[test]
    fn gets_tile_data_first_row_second_byte() {
        let mut ppu = get_test_ppu();
        ppu.memory.borrow_mut().write(0x8001, 0x11);
        let mut fetcher = BackgroundFetcher::new();
        fetcher.tile_attrs = Some(0);
        let result = fetcher.get_tile_data(&mut ppu, 0, true);
        assert_eq!(result, 0x11);
    }

    #[test]
    fn gets_tile_data_second_row_first_byte() {
        let mut ppu = get_test_ppu();
        ppu.memory.borrow_mut().write(0x8002, 0x11);
        ppu.memory.borrow_mut().write(LY_ADDRESS, 0x01);
        let mut fetcher = BackgroundFetcher::new();
        fetcher.tile_attrs = Some(0);
        let result = fetcher.get_tile_data(&mut ppu, 0, false);
        assert_eq!(result, 0x11);
    }

    #[test]
    fn gets_tile_data_seventh_row_second_byte() {
        let mut ppu = get_test_ppu();
        ppu.memory.borrow_mut().write(0x800F, 0x11);
        ppu.memory.borrow_mut().write(LY_ADDRESS, 0x07);
        let mut fetcher = BackgroundFetcher::new();
        fetcher.tile_attrs = Some(0);
        let result = fetcher.get_tile_data(&mut ppu, 0, true);
        assert_eq!(result, 0x11);
    }

    #[test]
    fn gets_tile_data_seventh_row_second_byte_vertically_flipped() {
        let mut ppu = get_test_ppu();
        ppu.memory.borrow_mut().write(0x8001, 0x11);
        ppu.memory.borrow_mut().write(LY_ADDRESS, 0x07);
        let mut fetcher = BackgroundFetcher::new();
        fetcher.tile_attrs = Some(0b01000000);
        let result = fetcher.get_tile_data(&mut ppu, 0, true);
        assert_eq!(result, 0x11);
    }

    #[test]
    fn gets_tile_data_from_second_vram_bank() {
        let mut ppu = get_test_ppu();
        let mut fetcher = BackgroundFetcher::new();
        fetcher.tile_attrs = Some(0b00001000);
        ppu.memory.borrow_mut().write(LY_ADDRESS, 0x01);
        ppu.memory.borrow_mut().write(VBK_ADDRESS, 1);
        ppu.memory.borrow_mut().write(0x8002, 0x11);
        ppu.memory.borrow_mut().write(VBK_ADDRESS, 0);
        let result = fetcher.get_tile_data(&mut ppu, 0, false);
        assert_eq!(result, 0x11);
    }

    #[test]
    fn gets_tile_data_from_second_block_of_tiles_unsigned_addressing() {
        let mut ppu = get_test_ppu();
        let mut fetcher = BackgroundFetcher::new();
        fetcher.tile_attrs = Some(0);
        ppu.memory.borrow_mut().write(0x8300, 0x11);
        let result = fetcher.get_tile_data(&mut ppu, 0x30, false);
        assert_eq!(result, 0x11);
    }

    #[test]
    fn get_pixels_from_tile_data_correct_ordering() {
        let fetcher = BackgroundFetcher::new();
        let pixels = fetcher.get_pixels_from_tile_data(0xFF, 0x00);
        for pix in pixels {
            assert_eq!(pix, 1);
        }
    }

    #[test]
    fn get_pixels_from_tile_data_every_value_in_row() {
        let fetcher = BackgroundFetcher::new();
        let pixels = fetcher.get_pixels_from_tile_data(0x7C, 0x56);
        assert_eq!(pixels, [0, 2, 3, 1, 3, 1, 3, 0]);
    }

    #[test]
    fn get_bg_tile_attributes_gets_correct_value() {
        let ppu = get_test_ppu();
        let mut fetcher = BackgroundFetcher::new();
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
        let fetcher = BackgroundFetcher::new();
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
        let fetcher = BackgroundFetcher::new();
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
        let fetcher = BackgroundFetcher::new();
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
        let fetcher = BackgroundFetcher::new();
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
        let mut fetcher = SpriteFetcher::new();
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
        let mut fetcher = SpriteFetcher::new();
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

    // #[test]
    // fn same_object_is_not_queued_more_than_once() {
    //     let mut ppu = get_test_ppu();
    //     ppu.memory.borrow_mut().write(LCDC_ADDRESS, 0b00000100);
    //     set_obj_y_pos(&mut ppu, 0, 16);
    //     ppu.memory.borrow_mut().write(0xFE01, 0x08);
    //     ppu.update(80); // Complete oam scan and transition to draw
    //     assert_eq!(ppu.objects_on_scanline[0], 0xFE00);
    //     let fetcher = SpriteFetcher::new();
    //     let mut draw = Draw::new();
    //     for _ in 0..12 {
    //         draw.tick(&mut ppu);
    //     }
    //     assert_eq!(draw.bg_fetcher.sprites_to_fetch[0], 0xFE00);
    //     for i in 1..draw.bg_fetcher.sprites_to_fetch.len() {
    //         assert_ne!(draw.bg_fetcher.sprites_to_fetch[i], 0xFE00);
    //     }
    // }
}
