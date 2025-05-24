use std::cell::RefCell;
use std::rc::Rc;

use sdl2::event::Event;
use sdl2::keyboard::Keycode;

use crate::mem_manager::MemManager;
use crate::memory::Memory;

const JOYP_ADDRESS: u16 = 0xFF00;

pub struct InputHandler {
    memory: Rc<RefCell<MemManager>>,
    action_selected: bool,
    direction_selected: bool,
    action_input: u8,
    direction_input: u8,
}

impl InputHandler {
    pub fn new(memory: Rc<RefCell<MemManager>>) -> Self {
        let input = InputHandler {
            memory,
            action_selected: false,
            direction_selected: false,
            action_input: 0x0F,
            direction_input: 0x0F,
        };
        input.memory.borrow_mut().force_write(JOYP_ADDRESS, 0xFF);
        input
    }

    pub fn update(&mut self) {
        self.check_action_or_dir();
        self.write_state();
    }

    fn check_action_or_dir(&mut self) {
        let joyp = self.memory.borrow().read(JOYP_ADDRESS);
        self.action_selected = joyp & 0b00100000 == 0;
        self.direction_selected = joyp & 0b00010000 == 0;
    }

    fn write_state(&mut self) {
        let mut data = 0b11111111;
        self.check_action_or_dir();
        if self.action_selected {
            data = data & 0b11011111 & (0xF0 | self.action_input);
        }
        if self.direction_selected {
            data = data & 0b11101111 & (0xF0 | self.direction_input);
        }
        //println!("{data}");

        self.memory.borrow_mut().force_write(JOYP_ADDRESS, data);
    }

    pub fn update_joypad(&mut self, e: Event) {
        match e {
            Event::Quit { .. }
            | Event::KeyDown {
                keycode: Some(Keycode::Escape),
                ..
            } => std::process::exit(0),
            Event::KeyDown {
                keycode: Some(k), ..
            } => {
                self.handle_keydown(k);
                // Queue interrupt
                let if_address = 0xFF0F;
                let if_value = self.memory.borrow().read(if_address);
                self.memory
                    .borrow_mut()
                    .write(if_address, if_value | 0b00010000);
            }
            Event::KeyUp {
                keycode: Some(k), ..
            } => {
                self.handle_keyup(k);
            }
            _ => {}
        }
    }

    fn handle_keydown(&mut self, k: Keycode) {
        // Set JOYP bit
        match k {
            Keycode::Z => self.action_input &= 0b11111101,
            Keycode::Left => self.direction_input &= 0b11111101,
            Keycode::X => self.action_input &= 0b11111110,
            Keycode::Right => self.direction_input &= 0b11111110,
            Keycode::Return => self.action_input &= 0b11110111,
            Keycode::Down => self.direction_input &= 0b11110111,
            Keycode::Backspace => self.action_input &= 0b11111011,
            Keycode::Up => self.direction_input &= 0b11111011,
            _ => (),
        }
    }

    fn handle_keyup(&mut self, k: Keycode) {
        match k {
            Keycode::Z => self.action_input |= 0b00000010,
            Keycode::Left => self.direction_input |= 0b00000010,
            Keycode::X => self.action_input |= 0b00000001,
            Keycode::Right => self.direction_input |= 0b00000001,
            Keycode::Return => self.action_input |= 0b00001000,
            Keycode::Down => self.direction_input |= 0b00001000,
            Keycode::Backspace => self.action_input |= 0b00000100,
            Keycode::Up => self.direction_input |= 0b00000100,
            _ => (),
        }
    }
}
