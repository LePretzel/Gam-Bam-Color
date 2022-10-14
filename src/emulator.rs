use std::cell::RefCell;
use std::rc::Rc;

use crate::cpu::CPU;
use crate::memory::MemManager;

pub struct Emulator {
    pub cpu: CPU,
    pub mem: Rc<RefCell<MemManager>>,
}

impl Emulator {
    pub fn run(&mut self) {}
}
