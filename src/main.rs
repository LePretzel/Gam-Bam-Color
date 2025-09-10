use clap::Parser;
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

// const SPHL_PATH: &str = "src/test_roms/sphl.gb";
// const MISC_PATH: &str = "src/test_roms/misc.gb";
// const LD_PATH: &str = "src/test_roms/ldrr.gb";
// const JP_PATH: &str = "src/test_roms/jp.gb";
// const RIMM_PATH: &str = "src/test_roms/rimm.gb";
// const BITOPS_PATH: &str = "src/test_roms/bitops.gb";
// const OPRR_PATH: &str = "src/test_roms/oprr.gb";
// const SPECIAL_PATH: &str = "src/test_roms/special.gb";
// const OPAHL_PATH: &str = "src/test_roms/opahl.gb";
// const INTERRUPT_PATH: &str = "src/test_roms/interrupts.gb";
const CPU_ROM_PATH: &str = "src/test_roms/cpu_full.gb";
// const INSTR_TIMING_PATH: &str = "src/test_roms/instr_timing.gb";

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    #[arg(short, long, default_value = CPU_ROM_PATH)]
    rom_path: String,
}

fn main() {
    let mut emulator = Emulator::new();
    let args = Args::parse();

    emulator.load_and_run(&args.rom_path);
}
