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
mod registers;
mod timer;

fn main() {
    // let sphl_path = "src/test_roms/sphl.gb";
    // let misc_path = "src/test_roms/misc.gb";
    // let ld_path = "src/test_roms/ldrr.gb";
    // let jp_path = "src/test_roms/jp.gb";
    // let rimm_path = "src/test_roms/rimm.gb";
    // let bitops_path = "src/test_roms/bitops.gb";
    // let oprr_path = "src/test_roms/oprr.gb";
    // let special_path = "src/test_roms/special.gb";
    // let opahl_path = "src/test_roms/opahl.gb";
    // let interrupt_path = "src/test_roms/interrupts.gb";
    let cpu_rom_path = "src/test_roms/cpu_full.gb";
    // let instr_timing_path = "src/test_roms/instr_timing.gb";

    let mut emulator = Emulator::new();
    emulator.load_and_run(cpu_rom_path);
}
