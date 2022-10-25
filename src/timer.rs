use std::{cell::RefCell, num::Wrapping, rc::Rc};

use crate::memory::{MemManager, Memory};

const BASE_SPEED: u32 = 4;
const DIV_ADDRESS: u16 = 0xFF04;
const TIMA_ADDRESS: u16 = 0xFF05;
const TMA_ADDRESS: u16 = 0xFF06;
const TAC_ADDRESS: u16 = 0xFF07;

// TODO: implement more of the obscure timer behavior
pub struct Timer {
    memory: Rc<RefCell<MemManager>>,
    passed_cycles_div: u32,
    passed_cycles_tima: u32,
    interrupt_ready: bool,
    set_to_tma_ready: bool,
}

impl Timer {
    pub fn new(memory: Rc<RefCell<MemManager>>) -> Self {
        let timer = Timer {
            memory,
            passed_cycles_div: 0,
            passed_cycles_tima: 0,
            interrupt_ready: false,
            set_to_tma_ready: false,
        };
        timer.memory.borrow_mut().write(TIMA_ADDRESS, 0x00);
        timer.memory.borrow_mut().write(TMA_ADDRESS, 0x00);
        timer.memory.borrow_mut().write(TAC_ADDRESS, 0xF8);

        timer
    }

    pub fn update(&mut self, cycles: u32) {
        self.passed_cycles_div += cycles;
        self.passed_cycles_tima += cycles;
        self.update_div();
        self.update_tima();
    }

    fn update_div(&mut self) {
        let div_speed = BASE_SPEED * 4;
        while self.passed_cycles_div >= div_speed {
            self.increment(DIV_ADDRESS);
            self.passed_cycles_div -= div_speed;
        }
    }

    fn get_tima_speed(&mut self) -> u32 {
        let tac = self.memory.borrow().read(TAC_ADDRESS);
        let speed = tac & 0b00000011;
        match speed {
            0b00 => 64,
            0b01 => 1,
            0b10 => 4,
            0b11 => 16,
            _ => 1,
        }
    }

    fn update_tima(&mut self) {
        let tac = self.memory.borrow().read(TAC_ADDRESS);
        if tac & 0b00000100 == 0 {
            return;
        };
        let tima_speed = BASE_SPEED * self.get_tima_speed();
        while self.passed_cycles_tima >= tima_speed {
            if self.memory.borrow().read(TIMA_ADDRESS) == 0xFF {
                self.interrupt_ready = true;
                self.set_to_tma_ready = true;
            }
            self.increment(TIMA_ADDRESS);
            self.passed_cycles_tima -= tima_speed;
            self.send_interrupt_if_ready(self.passed_cycles_tima);
            self.set_to_tma_if_ready(self.passed_cycles_tima);
        }
    }

    fn increment(&mut self, address: u16) {
        let mut curr = Wrapping(self.memory.borrow_mut().read(address));
        curr += 1;
        self.memory.borrow_mut().force_write(address, curr.0);
    }

    fn send_interrupt_if_ready(&mut self, remaining_cycles: u32) {
        if self.interrupt_ready && remaining_cycles >= 4 {
            const IF_ADDRESS: u16 = 0xFF0F;
            let flags = self.memory.borrow().read(IF_ADDRESS);
            self.memory
                .borrow_mut()
                .write(IF_ADDRESS, flags | 0b00000100);
            self.interrupt_ready = false;
        }
    }

    fn set_to_tma_if_ready(&mut self, remaining_cycles: u32) {
        if self.set_to_tma_ready && remaining_cycles >= 4 {
            let tma = self.memory.borrow().read(TMA_ADDRESS);
            self.memory.borrow_mut().write(TIMA_ADDRESS, tma);
            self.interrupt_ready = false;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn get_test_timer() -> Timer {
        Timer::new(Rc::new(RefCell::new(MemManager::new())))
    }

    fn read_div_and_tima(tim: Timer) -> (u8, u8) {
        let mem = tim.memory.borrow();
        let div = mem.read(DIV_ADDRESS);
        let tima = mem.read(TIMA_ADDRESS);
        (div, tima)
    }

    #[test]
    fn update_tima_base_speed_and_div_both_increment() {
        let mut tim = get_test_timer();
        tim.memory.borrow_mut().write(TAC_ADDRESS, 0b00000101);
        tim.update(16);
        assert_eq!(read_div_and_tima(tim), (0x01, 0x04));
    }

    #[test]
    fn update_tima_base_speed_and_div_both_increment_remaining_cycles() {
        let mut tim = get_test_timer();
        tim.memory.borrow_mut().write(TAC_ADDRESS, 0b00000101);
        tim.update(19);
        assert_eq!(read_div_and_tima(tim), (0x01, 0x04));
    }

    #[test]
    fn update_tima_continues_with_remaining_cycles() {
        let mut tim = get_test_timer();
        tim.memory.borrow_mut().write(TAC_ADDRESS, 0b00000101);
        tim.update(19);
        tim.update(1);
        assert_eq!(read_div_and_tima(tim), (0x01, 0x05));
    }

    #[test]
    fn update_tima_slowest_speed_div_increments() {
        let mut tim = get_test_timer();
        tim.memory.borrow_mut().write(TAC_ADDRESS, 0b00000100);
        tim.update(16);
        assert_eq!(read_div_and_tima(tim), (0x01, 0x00));
    }

    #[test]
    fn update_tima_slowest_speed_both_increment() {
        let mut tim = get_test_timer();
        tim.memory.borrow_mut().write(TAC_ADDRESS, 0b00000100);
        tim.update(256);
        assert_eq!(read_div_and_tima(tim), (0x10, 0x01));
    }

    #[test]
    fn tima_does_not_get_set_to_tma_if_not_enough_cycles_for_delay() {
        let mut tim = get_test_timer();
        tim.memory.borrow_mut().write(TAC_ADDRESS, 0b00000100);
        tim.memory.borrow_mut().write(TIMA_ADDRESS, 0xFF);
        tim.memory.borrow_mut().write(TMA_ADDRESS, 0x72);
        tim.update(256);
        assert_eq!(read_div_and_tima(tim), (0x10, 0x00));
    }

    #[test]
    fn tima_gets_set_to_tma_after_delay() {
        let mut tim = get_test_timer();
        tim.memory.borrow_mut().write(TAC_ADDRESS, 0b00000100);
        tim.memory.borrow_mut().write(TIMA_ADDRESS, 0xFF);
        tim.memory.borrow_mut().write(TMA_ADDRESS, 0x72);
        tim.update(260);
        assert_eq!(read_div_and_tima(tim), (0x10, 0x72));
    }

    #[test]
    fn tima_does_not_increment_if_timer_disabled() {
        let mut tim = get_test_timer();
        tim.memory.borrow_mut().write(TAC_ADDRESS, 0b00000001);
        tim.update(16);
        assert_eq!(read_div_and_tima(tim), (0x01, 0x00));
    }

    #[test]
    fn sends_interrupt() {
        let mut tim = get_test_timer();
        tim.memory.borrow_mut().write(TIMA_ADDRESS, 0xFF);
        tim.memory.borrow_mut().write(TAC_ADDRESS, 0b00000101);
        tim.update(8);
        let interrupt = (tim.memory.borrow().read(0xFF0F) & 0b00000100) >> 2;
        assert_eq!(interrupt, 1);
    }

    #[test]
    fn does_not_send_interrupt_if_timer_does_not_overflow() {
        let mut tim = get_test_timer();
        tim.memory.borrow_mut().write(TAC_ADDRESS, 0b00000101);
        tim.update(4);
        let interrupt = (tim.memory.borrow().read(0xFF0F) & 0b00000100) >> 2;
        assert_eq!(interrupt, 0);
    }

    #[test]
    fn does_not_send_interrupt_if_not_enough_cycles() {
        let mut tim = get_test_timer();
        tim.memory.borrow_mut().write(TIMA_ADDRESS, 0xFF);
        tim.memory.borrow_mut().write(TAC_ADDRESS, 0b00000101);
        tim.update(7);
        let interrupt = (tim.memory.borrow().read(0xFF0F) & 0b00000100) >> 2;
        assert_eq!(interrupt, 0);
    }
}
