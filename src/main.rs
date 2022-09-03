mod cpu;
mod memory;

fn main() {
    let mut cpu = cpu::CPU::new();

    cpu.load("./test_roms/ldrr.gb");
    cpu.run();
}
