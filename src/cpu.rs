#![allow(non_camel_case_types)]
use core::panic;

use crate::registers::RegisterBank;
use crate::memory::AddressSpace;
use crate::opcodes::{get_opcodes, Opcode};
use serde_json::Value;

const DEBUG: bool = true;


enum SingleDataLoc {
    B,
    C,
    D,
    E,
    H,
    L,
    HL_addr,
    A,
}

impl std::convert::From<u8> for SingleDataLoc {
    fn from(value: u8) -> Self {
        match value {
            0 => Self::B,
            1 => Self::C,
            2 => Self::D,
            3 => Self::E,
            4 => Self::H,
            5 => Self::L,
            6 => Self::HL_addr,
            7 => Self::A,
            _ => panic!("Tried to convert {} to SingleDataLoc (range 0-7)", value),
        }
    }
}

enum DoubleDataLoc {
    BC,
    DE,
    HL,
    AF,
    SP,
}

impl std::convert::From<u8> for DoubleDataLoc {
    fn from(value: u8) -> Self {
        match value {
            0 => Self::BC,
            1 => Self::DE,
            2 => Self::HL,
            3 => Self::AF,
            _ => Self::SP,
        }
    }
}

pub struct CPU {
    pub registers: RegisterBank,
    pub memory: AddressSpace,
    master_interrupt_enable: bool,
    clock: u64,
    opcodes: Value,
}

fn bytes_to_u16(extra_bytes: Vec<u8>) -> u16 {
    ((extra_bytes[1] as u16) << 8) | (extra_bytes[0] as u16)
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
            println!("{}", self);
            let opcode_byte = self.fetch();
            let (opcode_dict, opcode) = self.decode(opcode_byte);
            self.execute(opcode, opcode_dict);

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

    fn execute(&mut self, opcode: u16, opcode_dict: Opcode) {
        let start_clock_t = self.clock - 4;
        let code_length = match (opcode >> 8) & 0xFF {
            0xCB => opcode_dict.length - 2,
            _ => opcode_dict.length - 1,
        };

        let mut extra_bytes: Vec<u8> = Vec::new();
        for _ in 0..code_length {
            extra_bytes.push(self.fetch());
        }

        let remaining_cycles = opcode_dict.cycles[0] - ((self.clock - start_clock_t) as u8);

        if opcode == 0x0000 {
            if DEBUG {
                println!("> NOP");
            }
        } else if opcode == 0x00C3 {
            self.handle_jump_absolute_d16(opcode, extra_bytes);
        } else if 0x80 <= opcode && opcode < 0xC0 {
            self.handle_no_param_alu(opcode);
        } else {
            panic!("Unimplemented opcode: {:#06X}", opcode);
        }

        self.tick(remaining_cycles);
        if self.memory.read(0xFF02) == 0x81 {
            print!("{}", std::char::from_u32(self.memory.read(0xFF01) as u32).unwrap_or('?'));
            self.memory.write(0xFF02, 0);
        }
    }

    fn read_single(&self, src: SingleDataLoc) -> u8 {
        match src {
            SingleDataLoc::A => self.registers.A,
            SingleDataLoc::B => self.registers.B,
            SingleDataLoc::C => self.registers.C,
            SingleDataLoc::D => self.registers.D,
            SingleDataLoc::E => self.registers.E,
            SingleDataLoc::H => self.registers.H,
            SingleDataLoc::L => self.registers.L,
            SingleDataLoc::HL_addr => self.memory.read(self.registers.HL()),
        }
    }

    fn write_single(&mut self, dst: SingleDataLoc, value: u8) {
        match dst {
            SingleDataLoc::A => self.registers.A = value,
            SingleDataLoc::B => self.registers.B = value,
            SingleDataLoc::C => self.registers.C = value,
            SingleDataLoc::D => self.registers.D = value,
            SingleDataLoc::E => self.registers.E = value,
            SingleDataLoc::H => self.registers.H = value,
            SingleDataLoc::L => self.registers.L = value,
            SingleDataLoc::HL_addr => self.memory.write(self.registers.HL(), value),
        };
    }

    fn read_double(&self, src: DoubleDataLoc) -> u16 {
        match src {
            DoubleDataLoc::BC => self.registers.BC(),
            DoubleDataLoc::DE => self.registers.DE(),
            DoubleDataLoc::HL => self.registers.HL(),
            DoubleDataLoc::AF => self.registers.AF(),
            DoubleDataLoc::SP => self.registers.SP,
        }
    }

    fn write_double(&mut self, dst: DoubleDataLoc, value: u16) {
        match dst {
            DoubleDataLoc::BC => self.registers.set_BC(value),
            DoubleDataLoc::DE => self.registers.set_DE(value),
            DoubleDataLoc::HL => self.registers.set_HL(value),
            DoubleDataLoc::AF => self.registers.set_AF(value),
            DoubleDataLoc::SP => self.registers.SP = value,
        };
    }

    fn handle_jump_absolute_d16(&mut self, opcode: u16, extra_bytes: Vec<u8>) {
        let address = bytes_to_u16(extra_bytes);
        if DEBUG {
            println!("> JO nn ({:04X})", address);
        }
        self.registers.write_PC(address);
    }

    fn handle_no_param_alu(&mut self, opcode: u16) {
        let src_reg_i = opcode as u8 & 0x7;
        let srg_reg = SingleDataLoc::from(src_reg_i);
        let operand_value = self.read_single(srg_reg);
        let operand_repr = "n";

        if (opcode >> 3) & 0x7 == 0 {
            if DEBUG {
                println!("> ADD {operand_repr} ({operand_value:02X})");
            }
            todo!("ADD");
        } else if (opcode >> 3) & 0x7 == 0x1 {
            if DEBUG {
                println!("> ADC {operand_repr} ({operand_value:02X})");
            }
            todo!("ADC");
        } else if (opcode >> 3) & 0x7 == 0x2 {
            if DEBUG {
                println!("> SUB {operand_repr} ({operand_value:02X})");
            }
            todo!("SUB");
        } else if (opcode >> 3) & 0x7 == 0x3 {
            if DEBUG {
                println!("> SBC {operand_repr} ({operand_value:02X})");
            }
            todo!("SBC");
        } else if (opcode >> 3) & 0x7 == 0x4 {
            if DEBUG {
                println!("> AND {operand_repr} ({operand_value:02X})");
            }
            todo!("AND");
        } else if (opcode >> 3) & 0x7 == 0x5 {
            if DEBUG {
                println!("> XOR {operand_repr} ({operand_value:02X})");
            }
            todo!("XOR");
        } else if (opcode >> 3) & 0x7 == 0x6 {
            if DEBUG {
                println!("> OR {operand_repr} ({operand_value:02X})");
            }
            todo!("OR");
        } else if (opcode >> 3) & 0x7 == 0x7 {
            if DEBUG {
                println!("> CP {operand_repr} ({operand_value:02X})");
            }
            todo!("CP");
        } else {
            panic!("Unexpected opcode {opcode:02X}, expected generic ALU instruction!");
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
        write!(f, "AF: {:02X}, BC: {:02X}, DE: {:02X}, HL: {:02X}, SP: {:02X}, PC: {:02X}, F: {} | IME: {} | T: {} | LCDC: {:#04X} | STAT: {:#04X} | LY {:#04X}", 
         self.registers.AF(), self.registers.BC(), self.registers.DE(), self.registers.HL(), 
         self.registers.SP, self.registers.PC(), String::from_iter(flags), self.master_interrupt_enable, 
         self.clock, self.memory.read(0xFF40), self.memory.read(0xFF41), self.memory.read(0xFF44))
    }
}