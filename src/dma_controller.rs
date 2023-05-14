use std::{cell::RefCell, rc::Rc};

use crate::mem_manager::MemManager;
use crate::memory::Memory;

const OAM_DMA_SRC_ADDRESS: u16 = 0xFF46;
const OAM_START: u16 = 0xFE00;
const OAM_SIZE: u16 = 160;
const OAM_SRC_SENTINEL: u8 = 0xFF;
const OAM_DMA_TRANSFER_CYCLES: u32 = 640;

// Todo: Lock cpu memory access during OAM dma
pub struct DMAController {
    memory: Rc<RefCell<MemManager>>,
    oam_dma_is_active: bool,
    oam_dma_cycles_passed: u32,
}

impl DMAController {
    pub fn new(memory: Rc<RefCell<MemManager>>) -> Self {
        memory
            .borrow_mut()
            .write(OAM_DMA_SRC_ADDRESS, OAM_SRC_SENTINEL);
        Self {
            memory,
            oam_dma_is_active: false,
            oam_dma_cycles_passed: 0,
        }
    }

    pub fn update(&mut self, cycles: u32) {
        self.handle_oam_dma(cycles);
    }

    pub fn oam_dma_is_active(&self) -> bool {
        self.oam_dma_is_active
    }

    pub fn vram_dma_is_active(&self) -> bool {
        false
    }

    fn handle_oam_dma(&mut self, cycles: u32) {
        if self.oam_dma_is_active() {
            self.oam_dma_cycles_passed += cycles;
            if self.oam_dma_cycles_passed >= OAM_DMA_TRANSFER_CYCLES {
                self.oam_dma_is_active = false;
                self.oam_dma_cycles_passed = 0;
            } else {
                return;
            }
        }
        let source_value = self.memory.borrow().read(OAM_DMA_SRC_ADDRESS);
        if source_value != OAM_SRC_SENTINEL {
            assert!(source_value <= 0xDF);
            // Start transfer since the register has been written to
            self.oam_dma_is_active = true;
            // Should be ok to do the transfer all at once since all memory except hram is blocked
            // during transfer anyway
            let source_value = (source_value as u16) << 8;
            let mut mem = self.memory.borrow_mut();
            for i in 0..OAM_SIZE {
                let curr = mem.read(source_value + i);
                mem.write(OAM_START + i, curr);
            }

            mem.write(OAM_DMA_SRC_ADDRESS, OAM_SRC_SENTINEL);
        }
    }

    fn handle_vram_dma() {}
}

#[cfg(test)]
mod tests {
    use super::*;

    fn get_test_dma_controller() -> DMAController {
        let mem = Rc::new(RefCell::new(MemManager::new()));
        DMAController::new(mem.clone())
    }

    #[test]
    fn oam_dma_transfers_correctly() {
        let mut dma = get_test_dma_controller();
        dma.memory.borrow_mut().write(OAM_DMA_SRC_ADDRESS, 0);
        for i in 0..160 {
            dma.memory.borrow_mut().write(i, 0xAB);
        }
        assert_eq!(dma.oam_dma_is_active(), false);
        dma.handle_oam_dma(0);
        for i in 0xFE00..=0xFE9F {
            assert_eq!(dma.memory.borrow().read(i), 0xAB);
        }
        assert_eq!(dma.oam_dma_is_active(), true);
    }

    #[test]
    fn oam_dma_does_not_transfer_if_already_active() {
        let mut dma = get_test_dma_controller();
        dma.memory.borrow_mut().write(OAM_DMA_SRC_ADDRESS, 0);
        for i in 0..160 {
            dma.memory.borrow_mut().write(i, 0xAB);
        }
        dma.oam_dma_is_active = true;
        dma.handle_oam_dma(0);
        for i in 0xFE00..=0xFE9F {
            assert_eq!(dma.memory.borrow().read(i), 0x00);
        }
    }

    #[test]
    fn oam_dma_finishes_in_correct_amount_of_cycles() {
        let mut dma = get_test_dma_controller();
        dma.memory.borrow_mut().write(OAM_DMA_SRC_ADDRESS, 0);
        for i in 0..160 {
            dma.memory.borrow_mut().write(i, 0xAB);
        }
        dma.oam_dma_is_active = true;
        dma.handle_oam_dma(OAM_DMA_TRANSFER_CYCLES);
        for i in 0xFE00..=0xFE9F {
            assert_eq!(dma.memory.borrow().read(i), 0xAB);
        }
    }
}
