#![allow(non_camel_case_types)]
const FLAGS_C_BIT: u8 = 4;
const FLAGS_H_BIT: u8 = 5;
const FLAGS_N_BIT: u8 = 6;
const FLAGS_Z_BIT: u8 = 7;

pub struct RegisterBank {
    pub A: u8,
    F: u8,
    pub B: u8,
    pub C: u8,
    pub D: u8,
    pub E: u8,
    pub H: u8,
    pub L: u8,
    pub SP: u16,
    PC: u16,
}

impl RegisterBank {
    pub fn new() -> RegisterBank {
        RegisterBank {
            A: 0,
            F: 0,
            B: 0,
            C: 0,
            D: 0,
            E: 0,
            H: 0,
            L: 0,
            SP: 0xFFFE,
            PC: 0x0100,
        }
    }

    pub fn BC(&self) -> u16 {
        ((self.B as u16) << 8) | (self.C as u16)
    }

    pub fn DE(&self) -> u16 {
        ((self.D as u16) << 8) | (self.E as u16)
    }
    
    pub fn HL(&self) -> u16 {
        ((self.H as u16) << 8) | (self.L as u16)
    }

    pub fn AF(&self) -> u16 {
        ((self.A as u16) << 8) | (self.F as u16)
    }

    pub fn PC(&self) -> u16 {
        self.PC
    }

    pub fn increment_PC(&mut self) {
        self.PC = self.PC.wrapping_add(1);
    }

    pub fn set_BC(&mut self, v: u16) {
        self.B = ((v & 0xFF00) >> 8) as u8;
        self.C = (v & 0xFF) as u8;
    }

    pub fn set_DE(&mut self, v: u16) {
        self.D = ((v & 0xFF00) >> 8) as u8;
        self.E = (v & 0xFF) as u8;
    }
    pub fn set_HL(&mut self, v: u16) {
        self.H = ((v & 0xFF00) >> 8) as u8;
        self.L = (v & 0xFF) as u8;
    }
    pub fn set_AF(&mut self, v: u16) {
        self.A = ((v & 0xFF00) >> 8) as u8;
        self.F = (v & 0xF0) as u8;
    }

    pub fn write_PC(&mut self, v: u16) {
        self.PC = v
    }

    pub fn read_C(&self) -> bool {
        ((self.F >> FLAGS_C_BIT) & 1) == 1
    }

    pub fn read_H(&self) -> bool {
        ((self.F >> FLAGS_H_BIT) & 1) == 1
    }

    pub fn read_N(&self) -> bool {
        ((self.F >> FLAGS_N_BIT) & 1) == 1
    }

    pub fn read_Z(&self) -> bool {
        ((self.F >> FLAGS_Z_BIT) & 1) == 1
    }

    pub fn set_C(&mut self) {
        self.F |= 1 << FLAGS_C_BIT
    }

    pub fn set_H(&mut self) {
        self.F |= 1 << FLAGS_H_BIT
    }

    pub fn set_N(&mut self) {
        self.F |= 1 << FLAGS_N_BIT
    }

    pub fn set_Z(&mut self) {
        self.F |= 1 << FLAGS_Z_BIT
    }

    pub fn clear_C(&mut self) {
        self.F &= (1 << FLAGS_C_BIT) ^ 0xFF
    }

    pub fn clear_H(&mut self) {
        self.F |= (1 << FLAGS_H_BIT) ^ 0xFF
    }

    pub fn clear_N(&mut self) {
        self.F |= (1 << FLAGS_N_BIT) ^ 0xFF
    }

    pub fn clear_Z(&mut self) {
        self.F |= (1 << FLAGS_Z_BIT) ^ 0xFF
    }
}