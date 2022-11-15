pub trait Memory {
    fn read(&self, address: u16) -> u8;
    fn write(&mut self, address: u16, data: u8);
    fn read_u16(&self, address: u16) -> u16;
    fn write_u16(&mut self, address: u16, data: u16);
}

pub struct MemManager {
    memory: [u8; 0xFFFF + 1],
}

impl MemManager {
    pub fn new() -> Self {
        MemManager {
            memory: [0; 0xFFFF + 1],
        }
    }

    pub fn force_write(&mut self, address: u16, data: u8) {
        self.memory[address as usize] = data;
    }
}

const DIV_ADDRESS: u16 = 0xFF04;

impl Memory for MemManager {
    fn read(&self, address: u16) -> u8 {
        let data = self.memory[address as usize];
        data
    }

    fn write(&mut self, address: u16, data: u8) {
        match address {
            x if x == DIV_ADDRESS => self.memory[address as usize] = 0,
            _ => self.memory[address as usize] = data,
        }
    }

    fn read_u16(&self, address: u16) -> u16 {
        let low = self.read(address) as u16;
        let high = self.read(address + 1) as u16;
        (high << 8) | low
    }

    fn write_u16(&mut self, address: u16, data: u16) {
        let low = data as u8;
        let high = (data >> 8) as u8;
        self.write(address, low);
        self.write(address + 1, high);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writing_to_div_sets_it_to_zero() {
        let mut mem = MemManager::new();
        mem.write(DIV_ADDRESS, 0x45);
        assert_eq!(mem.read(DIV_ADDRESS), 0x00);
    }
}
