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
}

impl Memory for MemManager {
    fn read(&self, address: u16) -> u8 {
        let data = self.memory[address as usize];
        //println!("Read value {:x} from {:x}", data, address);
        data
    }

    fn write(&mut self, address: u16, data: u8) {
        //println!("Wrote value {:x} to {:x}", data, address);
        self.memory[address as usize] = data;
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
