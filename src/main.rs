use emulator::Emulator;

mod cpu;
mod dma_controller;
mod emulator;
mod fetcher;
mod input_handler;
mod mbc;
mod mem_manager;
mod memory;
mod ppu;
mod timer;

fn main() {
    // Currently passing
    let sphl_path = "src/test_roms/sphl.gb";
    let misc_path = "src/test_roms/misc.gb";
    let ld_path = "src/test_roms/ldrr.gb";
    let jp_path = "src/test_roms/jp.gb";
    let rimm_path = "src/test_roms/rimm.gb";
    let bitops_path = "src/test_roms/bitops.gb";
    let oprr_path = "src/test_roms/oprr.gb";
    let special_path = "src/test_roms/special.gb";
    let opahl_path = "src/test_roms/opahl.gb";
    let interrupt_path = "src/test_roms/interrupts.gb";
    let cpu_rom_path = "src/test_roms/cpu_full.gb";
    let instr_timing_path = "src/test_roms/instr_timing.gb";

    let mooneye_path = "src/test_roms/mooneye/mts/acceptance/";
    let mooneye_daa = "src/test_roms/daa.gb";
    let mooneye_ppu_hblank = mooneye_path.to_owned() + "ppu/hblank_ly_scx_timing-GS.gb";
    let mooneye_ppu_intr_1_2_timing = mooneye_path.to_owned() + "ppu/intr_1_2_timing-GS.gb";
    let mooneye_ppu_intr_2_0_timing = mooneye_path.to_owned() + "ppu/intr_2_0_timing.gb";
    let mooneye_ppu_mode_0_timing = mooneye_path.to_owned() + "ppu/intr_2_mode0_timing.gb";
    let mooneye_ppu_mode_0_timing_sprites =
        mooneye_path.to_owned() + "ppu/intr_2_mode0_timing_sprites.gb";

    let mealybug_bgp_change =
        "src/test_roms/complete/mealybug-tearoom-tests/ppu/m3_bgp_change_sprites.gb";
    let mealybug_lcdc_obj_change =
        "src/test_roms/complete/mealybug-tearoom-tests/ppu/m3_lcdc_obj_en_change.gb";
    let mealybug_lcdc_obj_change_variant =
        "src/test_roms/complete/mealybug-tearoom-tests/ppu/m3_lcdc_obj_en_change_variant.gb";
    let mealybug_obp0_change =
        "src/test_roms/complete/mealybug-tearoom-tests/ppu/m3_obp0_change.gb";

    let zelda_rom_path = "src/test_roms/zelda.gbc";
    let pokemon_rom_path = "src/test_roms/pokemon_red.gb";
    let pokemon_crystal_rom_path = "src/test_roms/crystal.gbc";
    let tetris_dx_path = "src/test_roms/tetrisdx.gbc";
    let tetris_path = "src/test_roms/tetris.gb";
    let mario_land_path = "src/test_roms/mario_land.gb";
    let mario_land2_path = "src/test_roms/mario_land2.gb";
    let mario_dx_path = "src/test_roms/mariodx.gbc";
    let dr_mario_path = "src/test_roms/dr_mario.gb";
    let wario_path = "src/test_roms/wario.gb";

    let cgb_acid = "src/test_roms/cgb-acid2.gbc";

    let mut emulator = Emulator::new();
    emulator.load_and_run(dr_mario_path);
}
