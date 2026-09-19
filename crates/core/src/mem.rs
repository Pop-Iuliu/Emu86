pub const MEM_SIZE: usize = 0x10_0000;

pub struct Memory {
    data: Box<[u8; MEM_SIZE]>,
}

impl Memory {
    pub fn new() -> Self {
        Self {
            data: Box::new([0; MEM_SIZE]),
        }
    }

    pub fn linear(seg: u16, off: u16) -> usize {
        (((seg as u32) << 4) + off as u32) as usize & (MEM_SIZE - 1)
    }

    pub fn read(&self, addr: usize) -> u8 {
        self.data[addr & (MEM_SIZE - 1)]
    }

    pub fn read_word(&self, addr: usize) -> u16 {
        (self.read(addr + 1) as u16) << 8 | self.read(addr) as u16
    }

    pub fn write(&mut self, addr: usize, v: u8) {
        self.data[addr & (MEM_SIZE - 1)] = v;
    }

    pub fn write_word(&mut self, addr: usize, v: u16) {
        self.write(addr, v as u8);
        self.write(addr + 1, (v >> 8) as u8);
    }

    pub fn load(&mut self, addr: usize, bytes: &[u8]) {
        for (i, b) in bytes.iter().enumerate() {
            self.write(addr + i, *b);
        }
    }
}

impl Default for Memory {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn linear_address_wraps_at_one_mebibyte() {
        assert_eq!(Memory::linear(0xFFFF, 0x0010), 0);
        assert_eq!(Memory::linear(0x0000, 0xFFFF), 0xFFFF);
        assert_eq!(Memory::linear(0xFFFF, 0x000F), 0xFFFFF);
    }

    #[test]
    fn writes_and_reads_wrap() {
        let mut m = Memory::new();
        m.write(MEM_SIZE, 0xAB);
        assert_eq!(m.read(0), 0xAB);
        m.write(0xFFFFF, 0xCD);
        m.load(MEM_SIZE, &[0x01, 0x02]);
        assert_eq!(m.read(0), 0x01);
        assert_eq!(m.read(1), 0x02);
        assert_eq!(m.read_word(0xFFFFF), 0x01CD);
    }
}
