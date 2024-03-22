#![allow(non_camel_case_types)]
use core::panic;

use crate::registers::RegisterBank;
use crate::memory::AddressSpace;
use crate::opcodes::{get_opcodes, Opcode};
use serde_json::Value;

pub struct CPU {
    pub registers: RegisterBank,
    pub memory: AddressSpace,
    master_interrupt_enable: bool,
    clock: u64,
    opcodes: Value,
}


impl CPU {
    pub fn new() -> CPU {
        CPU {
            registers: RegisterBank::new(),
            memory: AddressSpace::new(),
            master_interrupt_enable: false,
            clock: 0,
            opcodes: match get_opcodes() {
                Ok(x) => x,
                Err(x) => panic!("Couldn't load opcodes: {}", x),
            },
        }
    }

    fn tick(&mut self, nticks: u8) {
        self.clock += nticks as u64;
        for i in 0..nticks {
            self.memory.tick();
        }
    }

    fn boot(&mut self) {
        self.registers.set_AF(0x01B0);
        self.registers.set_BC(0x0013);
        self.registers.set_DE(0x00D8);
        self.registers.set_HL(0x014D);
        self.registers.SP = 0xFFFE;
        self.registers.write_PC(0x0100);

        self.memory.write(0xFF05, 0x00);  // TIMA
        self.memory.write(0xFF06, 0x00);  // TMA
        self.memory.write(0xFF07, 0x00);  // TAC
        self.memory.write(0xFF10, 0x80);  // NR10
        self.memory.write(0xFF11, 0xBF);  // NR11
        self.memory.write(0xFF12, 0xF3);  // NR12
        self.memory.write(0xFF14, 0xBF);  // NR14
        self.memory.write(0xFF16, 0x3F);  // NR21
        self.memory.write(0xFF17, 0x00);  // NR22
        self.memory.write(0xFF19, 0xBF);  // NR24
        self.memory.write(0xFF1A, 0x7F);  // NR30
        self.memory.write(0xFF1B, 0xFF);  // NR31
        self.memory.write(0xFF1C, 0x9F);  // NR32
        self.memory.write(0xFF1E, 0xBF);  // NR33
        self.memory.write(0xFF20, 0xFF);  // NR41
        self.memory.write(0xFF21, 0x00);  // NR42
        self.memory.write(0xFF22, 0x00);  // NR43
        self.memory.write(0xFF23, 0xBF);  // NR30
        self.memory.write(0xFF24, 0x77);  // NR50
        self.memory.write(0xFF25, 0xF3);  // NR51
        self.memory.write(0xFF26, 0xF1);  // NR52, GB, 0xF0-SGB
        self.memory.write(0xFF40, 0x91);  // LCDC
        self.memory.write(0xFF42, 0x00);  // SCY
        self.memory.write(0xFF43, 0x00);  // SCX
        self.memory.write(0xFF45, 0x00);  // LYC
        self.memory.write(0xFF47, 0xFC);  // BGP
        self.memory.write(0xFF48, 0xFF);  // OBP0
        self.memory.write(0xFF49, 0xFF);  // OBP1
        self.memory.write(0xFF4A, 0x00);  // WY
        self.memory.write(0xFF4B, 0x00);  // WX
        self.memory.write(0xFFFF, 0x00);  // IE
    }

    pub fn run(&mut self) {
        self.boot();
        loop {
            let opcode_byte = self.fetch();
            let (opcode_dict, opcode) = self.decode(opcode_byte);

            if self.clock > 10e6 as u64 {
                break;
            }
        }
    }

    fn fetch(&mut self) -> u8{
        let opcode = self.memory.read(self.registers.PC());
        self.registers.increment_PC();
        self.tick(4);
        opcode   
    }

    fn decode(&mut self, opcode_byte: u8) -> (Opcode, u16) {
        if opcode_byte == 0xCB {
            let opcode_lower = self.fetch() as u16;
            let opcode = ((opcode_byte as u16) << 8) | opcode_lower;
            let opcode_dict: Opcode = serde_json::from_value(self.opcodes["cbprefixed"][format!("0x{:02x}", opcode_byte)].to_owned()).unwrap();
            (opcode_dict, opcode)
        } else {
            println!("Extracting value from {}", format!("0x{:02x}", opcode_byte));
            let opcode_dict: Opcode = serde_json::from_value(self.opcodes["unprefixed"][format!("0x{:02x}", opcode_byte)].to_owned()).unwrap();
            (opcode_dict, opcode_byte as u16)
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