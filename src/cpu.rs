#![allow(non_camel_case_types)]
use crate::registers::RegisterBank;
use crate::memory::AddressSpace;

pub struct CPU {
    pub registers: RegisterBank,
    pub memory: AddressSpace,
}


impl CPU {
    pub fn new() -> CPU {
        CPU {
            registers: RegisterBank::new(),
            memory: AddressSpace::new(),
        }
    }
}

impl std::fmt::Display for CPU {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut flags = ['-', '-', '-', '-'];
        if self.registers.read_Z() {
            flags[0] = 'Z';
        }
        if self.registers.read_N() {
            flags[1] = 'N';
        }
        if self.registers.read_H() {
            flags[2] = 'H';
        }
        if self.registers.read_C() {
            flags[3] = 'C';
        }
        write!(f, "AF: {:02X}, BC: {:02X}, DE: {:02X}, HL: {:02X}, SP: {:02X}, PC: {:02X}, F: {}", self.registers.AF(), self.registers.BC(), self.registers.DE(), self.registers.HL(), self.registers.SP, self.registers.PC(), String::from_iter(flags))
    }
}