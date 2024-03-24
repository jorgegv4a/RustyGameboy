#![allow(non_camel_case_types)]
use core::panic;

use crate::registers::RegisterBank;
use crate::memory::AddressSpace;
use crate::opcodes::{get_opcodes, Opcode};
use serde_json::Value;

// const DEBUG: bool = false;
const DEBUG: bool = true;

#[derive(Debug)]
enum SingleDataLoc {
    B,
    C,
    D,
    E,
    H,
    L,
    HL_addr,
    A,
    n8(u8),
}

impl std::convert::From<(u8, Option<u8>)> for SingleDataLoc {
    fn from(value: (u8, Option<u8>)) -> Self {
        match value {
            (0, None) => Self::B,
            (1, None) => Self::C,
            (2, None) => Self::D,
            (3, None) => Self::E,
            (4, None) => Self::H,
            (5, None) => Self::L,
            (6, None) => Self::HL_addr,
            (7, None) => Self::A,
            (_, Some(x)) => Self::n8(x),
            (idx, None) => panic!("Tried to convert {} to SingleDataLoc (range 0-7)", idx),
        }
    }
}

#[derive(Debug, PartialEq)]
enum DoubleDataLoc {
    BC,
    DE,
    HL,
    SP,
    AF,
    n16(u16),
}

impl std::convert::From<(u8, Option<u16>)> for DoubleDataLoc {
    fn from(value: (u8, Option<u16>)) -> Self {
        match value {
            (0, _) => Self::BC,
            (1, _) => Self::DE,
            (2, _) => Self::HL,
            (3, _) => Self::SP,
            (_idx, None) => Self::AF,
            (_, Some(x)) => Self::n16(x),
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

fn byte_to_i16(in_value: u8) -> u16 {
    let mut value = in_value as u16;
    if (in_value >> 7) & 1 == 1 {
        value = 0xFF00 | (in_value as u16);
    }
    value
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
            if DEBUG {
                println!("{}", self);
            }
            let opcode_byte = self.fetch();
            let (opcode_dict, opcode) = self.decode(opcode_byte);
            self.execute(opcode, opcode_dict);

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

        let mut remaining_cycles = opcode_dict.cycles[0] - ((self.clock - start_clock_t) as u8);
        if opcode & 0xFF00 == 0 { 
            if opcode == 0x00 {
                if DEBUG {
                    println!("> NOP");
                }
            } else if opcode == 0x10 {
                if DEBUG {
                    println!("> STOP");
                }
            
            // LOADS
            } else if opcode == 0x08 {
                self.handle_load_from_SP_to_indirect_address(opcode, extra_bytes);

            } else if (0x40 <= opcode && opcode < 0x80) && opcode != 0x76 {
                self.handle_no_param_loads(opcode);

            } else if opcode & 0xC7 == 0x06 {
                self.handle_d8_loads(opcode, extra_bytes);

            } else if opcode & 0xCF == 0x01 {
                self.handle_load_d16_to_r16(opcode, extra_bytes);

            } else if opcode & 0xC7 == 0x02 {
                self.handle_indirect_loads(opcode);

            } else if opcode & 0xFE == 0xF8 {
                self.handle_load_r16_to_r16(opcode, extra_bytes);

            } else if opcode & 0xE5 == 0xE0 && opcode & 0xEF != 0xE8 {
                self.handle_misc_indirect_loads(opcode, extra_bytes);

            // JUMPS
            } else if opcode & 0xE7 == 0xC2 {
                let branch = self.handle_jump_d16_cond(opcode, extra_bytes);
                if !branch {
                    remaining_cycles = opcode_dict.cycles[1] - ((self.clock - start_clock_t) as u8);
                }

            } else if opcode == 0xC3 {
                self.handle_jump_absolute_d16(opcode, extra_bytes);

            } else if opcode == 0xE9 {
                self.handle_jump_absolute_HL(opcode, extra_bytes);

            } else if opcode & 0xE7 == 0x20 {
                let branch = self.handle_jump_relative_cond(opcode, extra_bytes);
                if !branch {
                    remaining_cycles = opcode_dict.cycles[1] - ((self.clock - start_clock_t) as u8);
                }
                
            } else if opcode == 0x18 {
                self.handle_jump_relative(opcode, extra_bytes);

            // ARITHMETIC/LOGIC
            } else if 0x80 <= opcode && opcode < 0xC0 {
                self.handle_u8_alu(opcode, Vec::new()); // r8

            } else if opcode & 0xC7 == 0xC6 {
                self.handle_u8_alu(opcode, extra_bytes); // n8

            } else if opcode & 0xE7 == 0x27 {
                self.handle_accumulator_misc(opcode);
                todo!("not done");
                
            } else if opcode & 0xC6 == 0x04 {
                self.handle_inc_dec_r8(opcode);
                
            
            } else if opcode & 0xC7 == 0x03 {
                self.handle_inc_dec_r16(opcode);

            } else if opcode & 0xCF == 0x09 {
                self.handle_add_r16(opcode);

            } else if opcode == 0xE8 {
                self.handle_add_SP_int8(opcode, extra_bytes);
            
            // STACK
            } else if opcode & 0xCF == 0xC1 {
                self.handle_r16_pop(opcode);

            } else if opcode & 0xCF == 0xC5 {
                self.handle_r16_push(opcode);

            } else if opcode & 0xE7 == 0x07 {
                self.handle_rotate_accumulator(opcode);

            // CALL/RESET/RETURN
            } else if opcode & 0xE7 == 0xC4 {
                self.handle_call_cond(opcode, extra_bytes);

            } else if opcode == 0xCD {
                self.handle_call_d16(opcode, extra_bytes);

            } else if opcode & 0xC7 == 0xC7 {
                self.handle_reset_vector(opcode);

            } else if opcode & 0xEF == 0xC9 {
                self.handle_return(opcode);
            
            } else if opcode & 0xE7 == 0xC0 {
                let branch = self.handle_return_cond(opcode);
                if !branch {
                    remaining_cycles = opcode_dict.cycles[1] - ((self.clock - start_clock_t) as u8);
                }

            // -- INTERRUPT CONTROL
            } else if opcode == 0xF3 { // DI
                if DEBUG {
                    println!("> DI");
                }
                self.master_interrupt_enable = false;

            } else if opcode == 0xFB { // EI
                if DEBUG {
                    println!("> EI");
                }
                self.master_interrupt_enable = true; // TODO: should be done after the next cycle, not immediately
            }
        } else if opcode & 0xFF00 == 0xCB00 {
            let low_opcode = (opcode & 0xFF) as u8;
            if low_opcode & 0xC0 == 0x00 {
                self.handle_no_params_shifts(low_opcode);

            } else if low_opcode & 0xC0 == 0x40 {
                self.handle_bit_test(low_opcode)

            } else if low_opcode & 0xC0 == 0x80 {
                self.handle_bit_clear(low_opcode);

            } else if low_opcode & 0xC0 == 0xC0 {
                self.handle_bit_set(low_opcode);
            } else {
                panic!("Unimplemented opcode: {:#06X}", opcode);
            }
        }

        self.tick(remaining_cycles);
        if self.memory.read(0xFF02) == 0x81 {
            // print!("{}", std::char::from_u32(self.memory.read(0xFF01) as u32).unwrap_or('?'));
            self.memory.write(0xFF02, 0);
        }
    }

    fn read_single(&self, src: &SingleDataLoc) -> u8 {
        match src {
            SingleDataLoc::A => self.registers.A,
            SingleDataLoc::B => self.registers.B,
            SingleDataLoc::C => self.registers.C,
            SingleDataLoc::D => self.registers.D,
            SingleDataLoc::E => self.registers.E,
            SingleDataLoc::H => self.registers.H,
            SingleDataLoc::L => self.registers.L,
            SingleDataLoc::HL_addr => self.memory.read(self.registers.HL()),
            SingleDataLoc::n8(x) => *x,
        }
    }

    fn write_single(&mut self, dst: &SingleDataLoc, value: u8) {
        match dst {
            SingleDataLoc::A => self.registers.A = value,
            SingleDataLoc::B => self.registers.B = value,
            SingleDataLoc::C => self.registers.C = value,
            SingleDataLoc::D => self.registers.D = value,
            SingleDataLoc::E => self.registers.E = value,
            SingleDataLoc::H => self.registers.H = value,
            SingleDataLoc::L => self.registers.L = value,
            SingleDataLoc::HL_addr => self.memory.write(self.registers.HL(), value),
            SingleDataLoc::n8(_) => panic!("Cannot write to u8 immediate"),
        };
    }

    fn read_double(&self, src: &DoubleDataLoc) -> u16 {
        match src {
            DoubleDataLoc::BC => self.registers.BC(),
            DoubleDataLoc::DE => self.registers.DE(),
            DoubleDataLoc::HL => self.registers.HL(),
            DoubleDataLoc::AF => self.registers.AF(),
            DoubleDataLoc::SP => self.registers.SP,
            DoubleDataLoc::n16(x) => *x,
        }
    }

    fn write_double(&mut self, dst: &DoubleDataLoc, value: u16) {
        match dst {
            DoubleDataLoc::BC => self.registers.set_BC(value),
            DoubleDataLoc::DE => self.registers.set_DE(value),
            DoubleDataLoc::HL => self.registers.set_HL(value),
            DoubleDataLoc::AF => self.registers.set_AF(value),
            DoubleDataLoc::SP => self.registers.SP = value,
            DoubleDataLoc::n16(_) => panic!("Cannot write to u16 immediate"),
        };
    }

    fn handle_jump_d16_cond(&mut self, opcode: u16, extra_bytes: Vec<u8>) -> bool {
        let (condition, cond_repr) = match (opcode >> 3) & 0x3 {
            0 => (!self.registers.read_flag_Z(), "NZ"),
            1 => (self.registers.read_flag_Z(), "Z"),
            2 => (!self.registers.read_flag_C(), "NC"),
            3 => (self.registers.read_flag_C(), "C"),
            _ => unreachable!("What?")
        };
        let address = bytes_to_u16(extra_bytes);
        if DEBUG {
            println!("> JP {cond_repr}, nn ({address:04X})");
        }
        if !condition {
            return false;
        }
        self.registers.write_PC(address);
        return true
    }

    fn handle_jump_absolute_d16(&mut self, opcode: u16, extra_bytes: Vec<u8>) {
        let address = bytes_to_u16(extra_bytes);
        if DEBUG {
            println!("> JP nn ({:04X})", address);
        }
        self.registers.write_PC(address);
    }

    fn handle_jump_absolute_HL(&mut self, opcode: u16, extra_bytes: Vec<u8>) {
        if DEBUG {
            println!("> JP HL");
        }
        let address = self.read_double(&DoubleDataLoc::HL);
        self.registers.write_PC(address);
    }

    fn handle_jump_relative_cond(&mut self, opcode: u16, extra_bytes: Vec<u8>) -> bool {
        let (condition, cond_repr) = match (opcode >> 3) & 0x3 {
            0 => (!self.registers.read_flag_Z(), "NZ"),
            1 => (self.registers.read_flag_Z(), "Z"),
            2 => (!self.registers.read_flag_C(), "NC"),
            3 => (self.registers.read_flag_C(), "C"),
            _ => unreachable!("What?")
        };
        let immediate = byte_to_i16(extra_bytes[0]);
        if DEBUG {
            println!("> JP {cond_repr}, e ({immediate:02X})");
        }
        if !condition {
            return false;
        }
        let address = self.registers.PC().wrapping_add(immediate);
        self.registers.write_PC(address);
        return true
    }

    fn handle_jump_relative(&mut self, opcode: u16, extra_bytes: Vec<u8>) {
        let immediate = byte_to_i16(extra_bytes[0]);
        if DEBUG {
            println!("> JP e ({immediate:02X})");
        }
        let address = self.registers.PC().wrapping_add(immediate);
        self.registers.write_PC(address);
    }

    fn add_u8(&mut self, operand: u8, with_carry: bool) {
        let value_pre = self.registers.A;
        self.registers.A = value_pre.wrapping_add(operand);
        if with_carry {
            self.registers.A = value_pre.wrapping_add(1);
        }
        self.registers.flag_C_from_bool((value_pre as u16) + (operand as u16) + (with_carry as u16) > 0xFF);
        self.registers.flag_H_from_bool((value_pre & 0xF) + (operand & 0xF) + (with_carry as u8) > 0xF);
        self.registers.clear_flag_N();
        self.registers.flag_Z_from_bool(self.registers.A == 0);
    }

    fn subtract_u8(&mut self, operand: u8, with_carry: bool) {
        let value_pre = self.registers.A;
        self.registers.A = value_pre.wrapping_sub(operand);
        if with_carry {
            self.registers.A = value_pre.wrapping_add(1);
        }
        self.registers.flag_C_from_bool((value_pre as u16) < (operand as u16) + (with_carry as u16));
        self.registers.flag_H_from_bool((value_pre & 0xF) < (operand & 0xF) + (with_carry as u8));
        self.registers.set_flag_N();
        self.registers.flag_Z_from_bool(self.registers.A == 0);
    }

    fn and_u8(&mut self, operand: u8) {
        self.registers.A &= operand;
        self.registers.clear_flag_C();
        self.registers.set_flag_H();
        self.registers.clear_flag_N();
        self.registers.flag_Z_from_bool(self.registers.A == 0);
    }

    fn xor_u8(&mut self, operand: u8) {
        self.registers.A ^= operand;
        self.registers.clear_flag_C();
        self.registers.clear_flag_H();
        self.registers.clear_flag_N();
        self.registers.flag_Z_from_bool(self.registers.A == 0);
    }

    fn or_u8(&mut self, operand: u8) {
        self.registers.A |= operand;
        self.registers.clear_flag_C();
        self.registers.clear_flag_H();
        self.registers.clear_flag_N();
        self.registers.flag_Z_from_bool(self.registers.A == 0);
    }

    fn compare_u8(&mut self, operand: u8) {
        let value_pre = self.registers.A;
        let output = value_pre.wrapping_sub(operand);
        self.registers.flag_C_from_bool(value_pre < operand);
        self.registers.flag_H_from_bool((value_pre & 0xF) < (operand & 0xF));
        self.registers.set_flag_N();
        self.registers.flag_Z_from_bool(output == 0);
    }

    fn handle_u8_alu(&mut self, opcode: u16, extra_bytes: Vec<u8>) {
        let srg_reg: SingleDataLoc;
        if extra_bytes.len() == 0 {
            let src_reg_i = opcode as u8 & 0x7;
            srg_reg = SingleDataLoc::from((src_reg_i, None));
        } else {
            let immediate = extra_bytes[0];
            srg_reg = SingleDataLoc::from((0, Some(immediate)));
        }
        let operand_value = self.read_single(&srg_reg);

        if (opcode >> 3) & 0x7 == 0 {
            if DEBUG {
                println!("> ADD {srg_reg:?} ({operand_value:02X})");
            }
            self.add_u8(operand_value, false);

        } else if (opcode >> 3) & 0x7 == 0x1 {
            if DEBUG {
                println!("> ADC {srg_reg:?} ({operand_value:02X})");
            }
            if self.registers.read_flag_C() {
                self.add_u8(operand_value, true);
            } else {
                self.add_u8(operand_value, false);
            }
        } else if (opcode >> 3) & 0x7 == 0x2 {
            if DEBUG {
                println!("> SUB {srg_reg:?} ({operand_value:02X})");
            }
            self.subtract_u8(operand_value, false);

        } else if (opcode >> 3) & 0x7 == 0x3 {
            if DEBUG {
                println!("> SBC {srg_reg:?} ({operand_value:02X})");
            }
            if self.registers.read_flag_C() {
                self.subtract_u8(operand_value, true);
            } else {
                self.subtract_u8(operand_value, false);
            }

        } else if (opcode >> 3) & 0x7 == 0x4 {
            if DEBUG {
                println!("> AND {srg_reg:?} ({operand_value:02X})");
            }
            self.and_u8(operand_value)
        } else if (opcode >> 3) & 0x7 == 0x5 {
            if DEBUG {
                println!("> XOR {srg_reg:?} ({operand_value:02X})");
            }
            self.xor_u8(operand_value)
        } else if (opcode >> 3) & 0x7 == 0x6 {
            if DEBUG {
                println!("> OR {srg_reg:?} ({operand_value:02X})");
            }
            self.or_u8(operand_value)
        } else if (opcode >> 3) & 0x7 == 0x7 {
            if DEBUG {
                println!("> CP {srg_reg:?} ({operand_value:02X})");
            }
            self.compare_u8(operand_value)
        } else {
            panic!("Unexpected opcode {opcode:02X}, expected generic ALU instruction!");
        }
    }

    fn handle_inc_dec_r8(&mut self, opcode: u16) {
        let src_reg_i = (opcode >> 3) as u8 & 0x7;
        let src_reg: SingleDataLoc = SingleDataLoc::from((src_reg_i, None));
        let operand_value = self.read_single(&src_reg);
        let increment_op = opcode & 1 == 0;
        let new_value: u8;
        let overflow: bool;

        if increment_op {
            (new_value, overflow) = operand_value.overflowing_add(1);
            self.write_single(&src_reg, new_value);
            self.registers.flag_H_from_bool((operand_value & 0xF) == 0xF);
            if DEBUG {
                println!("> INC {src_reg:?}");
            }
        } else {
            (new_value, overflow) = operand_value.overflowing_sub(1);
            self.write_single(&src_reg, new_value);
            self.registers.flag_H_from_bool((operand_value & 0xF) == 0);
            if DEBUG {
                println!("> DEC {src_reg:?}");
            }
        }
        self.registers.flag_N_from_bool(!increment_op);
        self.registers.flag_Z_from_bool(new_value == 0);
        self.write_single(&src_reg, new_value)
    }

    fn handle_inc_dec_r16(&mut self, opcode: u16) {
        let src_reg_i = ((opcode >> 4) as u8) & 0x3;
        let src_reg: DoubleDataLoc = DoubleDataLoc::from((src_reg_i, None));
        let operand_value = self.read_double(&src_reg);
        let increment_op = (opcode >> 3) & 1 == 0;
        let new_value: u16;

        if increment_op {
            new_value = operand_value.wrapping_add(1);
            if DEBUG {
                println!("> INC {src_reg:?}");
            }
        } else {
            new_value = operand_value.wrapping_sub(1);
            if DEBUG {
                println!("> DEC {src_reg:?}");
            }
        }
        self.write_double(&src_reg, new_value);
    }

    fn handle_add_r16(&mut self, opcode: u16) {
        let src_reg_i = ((opcode >> 4) as u8) & 0x3;
        let src_reg: DoubleDataLoc = DoubleDataLoc::from((src_reg_i, None));
        let operand_value = self.read_double(&src_reg);
        if DEBUG {
            println!("> ADD HL, {src_reg:?}");
        }
        let value_pre = self.registers.HL();
        let (new_value, overflow) = value_pre.overflowing_add(operand_value);
        self.write_double(&DoubleDataLoc::HL, new_value);
        self.registers.flag_C_from_bool(overflow);
        let low_reg_carry = ((value_pre & 0xFF) + (operand_value & 0xFF) > 0xFF) as u16;
        self.registers.flag_H_from_bool(((value_pre >> 8) & 0xF) + ((operand_value >> 8) & 0xF) + low_reg_carry > 0xF);
        self.registers.clear_flag_N();

    }

    fn handle_add_SP_int8(&mut self, opcode: u16, extra_bytes: Vec<u8>) {
        let immediate = byte_to_i16(extra_bytes[0]);
        if DEBUG {
            println!("> ADD SP, e ({immediate:02X})");
        }
        let value_pre = self.registers.SP;
        let (new_value, overflow) = self.registers.SP.overflowing_add(immediate);
        self.write_double(&DoubleDataLoc::SP, new_value);
        self.registers.flag_C_from_bool(overflow);
        self.registers.flag_H_from_bool((value_pre & 0xF) + (immediate & 0xF) > 0xF);
        self.registers.clear_flag_Z();
        self.registers.clear_flag_N();
    }

    fn handle_load_from_SP_to_indirect_address(&mut self, opcode: u16, extra_bytes: Vec<u8>) {
        let immediate = bytes_to_u16(extra_bytes);
        if DEBUG {
            println!("> LD (a16), SP ({immediate:04X})")
        }
        self.memory.write(immediate, (self.registers.PC() & 0xFF) as u8);
        self.memory.write(immediate + 1, ((self.registers.PC() >> 8) & 0xFF) as u8);
    }

    fn handle_no_param_loads(&mut self, opcode: u16) {
        let src_reg_i = opcode as u8 & 0x7;
        let src_reg: SingleDataLoc = SingleDataLoc::from((src_reg_i, None));
        let dst_reg_i = (opcode >> 3) as u8 & 0x7;
        let dst_reg: SingleDataLoc = SingleDataLoc::from((dst_reg_i, None));
        if DEBUG {
            println!("> LD {dst_reg:?}, {src_reg:?}");
        }
        
        let value = self.read_single(&src_reg);
        self.write_single(&dst_reg, value);
    }

    fn handle_d8_loads(&mut self, opcode: u16, extra_bytes: Vec<u8>) {
        let immediate = extra_bytes[0];
        let dst_reg_i = (opcode >> 3) as u8 & 0x7;
        let dst_reg: SingleDataLoc = SingleDataLoc::from((dst_reg_i, None));
        if DEBUG {
            println!("> LD {dst_reg:?}, n ({immediate:02X})");
        }
        self.write_single(&dst_reg, immediate);
    }

    fn handle_load_d16_to_r16(&mut self, opcode: u16, extra_bytes: Vec<u8>) {
        let immediate = bytes_to_u16(extra_bytes);
        let dst_reg_i = (opcode >> 4) as u8 & 0x3;
        let dst_reg: DoubleDataLoc = DoubleDataLoc::from((dst_reg_i, None));
        if DEBUG {
            println!("> LD {dst_reg:?}, d16 ({immediate:04X})")
        }
        self.write_double(&dst_reg, immediate);
    }

    fn handle_indirect_loads(&mut self, opcode: u16) {
        let dst_reg: DoubleDataLoc;
        if (opcode >> 5) & 1 == 1 {
            dst_reg = DoubleDataLoc::HL;
        } else {
            let dst_reg_i = ((opcode >> 4) as u8) & 0x3;
            dst_reg = DoubleDataLoc::from((dst_reg_i, None));
        }

        let load_to_acc = (opcode >> 3) & 1 == 1;
        if load_to_acc { // LD A, (r16)
            let src_address = self.read_double(&dst_reg);
            let value = self.memory.read(src_address);
            self.write_single(&SingleDataLoc::A, value);
        } else { // LD (r16), A
            let value = self.registers.A;
            let dst_address = self.read_double(&dst_reg);
            self.memory.write(dst_address, value)
        }
        if dst_reg == DoubleDataLoc::HL {
            if (opcode >> 4) & 1 == 0 {
                let value = self.read_double(&dst_reg).wrapping_add(1);
                self.write_double(&dst_reg, value);
                if DEBUG {
                    if load_to_acc {
                        println!("> LD A, (HL+)");
                    } else {
                        println!("> LD (HL+), A");
                    }
                }
            } else {
                let value = self.read_double(&dst_reg).wrapping_sub(1);
                self.write_double(&dst_reg, value);
                if DEBUG {
                    if load_to_acc {
                        println!("> LD A, (HL-)");
                    } else {
                        println!("> LD (HL-), A");
                    }
                }
            }
        } else {
            if DEBUG {
                if load_to_acc {
                    println!("> LD A, ({dst_reg:?})");
                } else {
                    println!("> LD ({dst_reg:?}), A");
                }
            }
        }
    }

    fn handle_load_r16_to_r16(&mut self, opcode: u16, extra_bytes: Vec<u8>) {
        if opcode & 1 == 0 {
            let immediate = byte_to_i16(extra_bytes[0]);
            println!("> LD HL, SP+e ({immediate:02X})");
            let value_pre = self.registers.SP;
            let (result, overflow) = self.registers.SP.overflowing_add(immediate);
            self.write_double(&DoubleDataLoc::HL, result);
            self.registers.flag_C_from_bool(overflow);
            self.registers.flag_H_from_bool((value_pre & 0xF) + (immediate & 0xF) > 0xF);
            self.registers.clear_flag_Z();
            self.registers.clear_flag_N();
        }
    }

    fn handle_misc_indirect_loads(&mut self, opcode: u16, extra_bytes: Vec<u8>) {
        if (opcode >> 1) & 1 == 1 {
            let address: u16;
            if (opcode >> 3) & 1 == 0 {
                address = 0xFF00 | (self.registers.C as u16);
            } else {
                address = bytes_to_u16(extra_bytes);
            }

            if (opcode >> 4) & 0x1 == 0 {
                if DEBUG {
                    if (opcode >> 3) & 1 == 0 {
                        println!("> LDH (C), A");
                    } else {
                        println!("> LD (nn), A ({address:04X})");
                    }
                }
                self.memory.write(address, self.registers.A);
            } else {
                if DEBUG {
                    if (opcode >> 3) & 1 == 0 {
                        println!("> LDH A, (C)");
                    } else {
                        println!("> LD A, (nn) ({address:04X})");
                    }
                }
                self.registers.A = self.memory.read(address);
            }
        } else {
            let address = 0xFF00 | (extra_bytes[0] as u16);
            if (opcode >> 4) & 1 == 0 {
                if DEBUG {
                    println!("> LDH (n), A ({:02X})", extra_bytes[0]);
                }
                self.memory.write(address, self.registers.A);
            } else {
                if DEBUG {
                    println!("> LDH A, (n) ({:02X})", extra_bytes[0]);
                }
                self.registers.A = self.memory.read(address);
            }
        }
    }

    fn pop_stack(&mut self) -> u16 {
        let mut sp = self.read_double(&DoubleDataLoc::SP);
        let mut value: u16 = self.memory.read(sp) as u16;
        sp = sp.wrapping_add(1);
        self.write_double(&DoubleDataLoc::SP, sp);

        value |= (self.memory.read(sp) as u16) << 8;
        sp = sp.wrapping_add(1);
        self.write_double(&DoubleDataLoc::SP, sp);
        value

    }

    fn handle_r16_pop(&mut self, opcode: u16) {
        let src_reg_i = ((opcode >> 4) as u8) & 0x3;        
        let mut src_reg: DoubleDataLoc = DoubleDataLoc::from((src_reg_i, None));
        if src_reg == DoubleDataLoc::SP {
            src_reg = DoubleDataLoc::AF;
        }
        let operand_value = self.read_double(&src_reg);
        if DEBUG {
            println!("> POP, {src_reg:?}");
        }

        let value = self.pop_stack();
        self.write_double(&src_reg, value);
    }

    fn push_stack(&mut self, value: u16) {
        let mut sp = self.read_double(&DoubleDataLoc::SP);
        sp = sp.wrapping_sub(1);
        self.write_double(&DoubleDataLoc::SP, sp);
        self.memory.write(sp, ((value >> 8) & 0xFF) as u8);

        sp = sp.wrapping_sub(1);
        self.write_double(&DoubleDataLoc::SP, sp);
        self.memory.write(sp, (value & 0xFF) as u8);

    }

    fn handle_r16_push(&mut self, opcode: u16) {
        let src_reg_i = ((opcode >> 4) as u8) & 0x3;        
        let mut src_reg: DoubleDataLoc = DoubleDataLoc::from((src_reg_i, None));
        if src_reg == DoubleDataLoc::SP {
            src_reg = DoubleDataLoc::AF;
        }
        if DEBUG {
            println!("> PUSH, {src_reg:?}");
        }

        let value = self.read_double(&src_reg);
        self.push_stack(value);
    }

    fn handle_rotate_accumulator(&mut self, opcode: u16) {
        if (opcode >> 3) & 0x3 == 0x0 {
            if DEBUG {
                println!("> RLCA");
            }
            let top_bit = (self.registers.A >> 7) & 1;
            self.registers.A = ((self.registers.A & 0x7F) << 1) | top_bit;
            self.registers.flag_C_from_bool(top_bit > 0);
        } else if (opcode >> 3) & 0x3 == 0x1 {
            if DEBUG {
                println!("> RRCA");
            }
            let bottom_bit = self.registers.A & 1;
            self.registers.A = (self.registers.A >> 1) | (bottom_bit << 7);
            self.registers.flag_C_from_bool(bottom_bit > 0);
        } else if (opcode >> 3) & 0x3 == 0x2 {
            if DEBUG {
                println!("> RLA");
            }
            let top_bit = (self.registers.A >> 7) & 1;
            self.registers.A = ((self.registers.A & 0x7F) << 1) | (self.registers.read_flag_C() as u8);
            self.registers.flag_C_from_bool(top_bit > 0);
        } else if (opcode >> 3) & 0x3 == 0x3 {
            if DEBUG {
                println!("> RRA");
            }
            let bottom_bit = self.registers.A & 1;
            self.registers.A = (self.registers.A >> 1) | ((self.registers.read_flag_C() as u8) << 7);
            self.registers.flag_C_from_bool(bottom_bit > 0);
        }
        self.registers.clear_flag_Z();
        self.registers.clear_flag_N();
        self.registers.clear_flag_H();
    }

    fn handle_call_cond(&mut self, opcode: u16, extra_bytes: Vec<u8>) -> bool {
        let (condition, cond_repr) = match (opcode >> 3) & 0x3 {
            0 => (!self.registers.read_flag_Z(), "NZ"),
            1 => (self.registers.read_flag_Z(), "Z"),
            2 => (!self.registers.read_flag_C(), "NC"),
            3 => (self.registers.read_flag_C(), "C"),
            _ => unreachable!("What?")
        };
        let address = bytes_to_u16(extra_bytes);
        if DEBUG {
            println!("> CALL {cond_repr}, nn ({address:04X})");
        }
        if !condition {
            return false;
        }
        self.push_stack(self.registers.PC());
        self.registers.write_PC(address);
        return true
    }

    fn handle_call_d16(&mut self, opcode: u16, extra_bytes: Vec<u8>) {
        let address = bytes_to_u16(extra_bytes);
        if DEBUG {
            println!("> CALL nn ({address:04X})");
        }
        self.push_stack(self.registers.PC());
        self.registers.write_PC(address);
    }

    fn handle_reset_vector(&mut self, opcode: u16) {
        let dst_address = ((opcode >> 3) & 0x7) * 8;
        if DEBUG {
            println!("> RST ({dst_address:#04X})");
        }
        self.push_stack(self.registers.PC());
        self.registers.write_PC(dst_address);
    }

    fn handle_return(&mut self, opcode: u16) {
        let address = self.pop_stack();
        self.registers.write_PC(address);
        if (opcode >> 4) & 1 == 1 {
            if DEBUG {
                println!("> RETI");
            }
            self.master_interrupt_enable = true;
        } else {
            if DEBUG {
                println!("> RET");
            }
        }
    }

    fn handle_return_cond(&mut self, opcode: u16) -> bool {
        let (condition, cond_repr) = match (opcode >> 3) & 0x3 {
            0 => (!self.registers.read_flag_Z(), "NZ"),
            1 => (self.registers.read_flag_Z(), "Z"),
            2 => (!self.registers.read_flag_C(), "NC"),
            3 => (self.registers.read_flag_C(), "C"),
            _ => unreachable!("What?")
        };
        if DEBUG {
            println!("> RET {cond_repr}");
        }
        if !condition {
            return false;
        }
        let address = self.pop_stack();
        self.registers.write_PC(address);
        return true;
    }

    fn rotate_left_circular(&mut self, src: SingleDataLoc) {
        let operand_value = self.read_single(&src);
        let top_bit = (operand_value >> 7) & 1;
        let output_value = ((operand_value & 0x7F) << 1) | top_bit;
        self.write_single(&src, output_value);
        self.registers.flag_C_from_bool(top_bit > 0);
        self.registers.flag_Z_from_bool(output_value == 0);
        self.registers.clear_flag_N();
        self.registers.clear_flag_H();
    }

    fn rotate_right_circular(&mut self, src: SingleDataLoc) {
        let operand_value = self.read_single(&src);
        let bottom_bit = operand_value & 1;
        let output_value = (operand_value >> 1) | (bottom_bit << 7);
        self.write_single(&src, output_value);
        self.registers.flag_C_from_bool(bottom_bit > 0);
        self.registers.flag_Z_from_bool(output_value == 0);
        self.registers.clear_flag_N();
        self.registers.clear_flag_H();
    }

    fn rotate_left(&mut self, src: SingleDataLoc) {
        let operand_value = self.read_single(&src);
        let top_bit = (operand_value >> 7) & 1;
        let output_value = ((operand_value & 0x7F) << 1) | (self.registers.read_flag_C() as u8);
        self.write_single(&src, output_value);
        self.registers.flag_C_from_bool(top_bit > 0);
        self.registers.flag_Z_from_bool(output_value == 0);
        self.registers.clear_flag_N();
        self.registers.clear_flag_H();
    }

    fn rotate_right(&mut self, src: SingleDataLoc) {
        let operand_value = self.read_single(&src);
        let bottom_bit = operand_value & 1;
        let output_value = (operand_value >> 1) | ((self.registers.read_flag_C() as u8) << 7);
        self.write_single(&src, output_value);
        self.registers.flag_C_from_bool(bottom_bit > 0);
        self.registers.flag_Z_from_bool(output_value == 0);
        self.registers.clear_flag_N();
        self.registers.clear_flag_H();
    }

    fn shift_left_arith(&mut self, src: SingleDataLoc) {
        let operand_value = self.read_single(&src);
        let top_bit = (operand_value >> 7) & 1;
        let output_value = ((operand_value & 0x7F) << 1);
        self.write_single(&src, output_value);
        self.registers.flag_C_from_bool(top_bit > 0);
        self.registers.flag_Z_from_bool(output_value == 0);
        self.registers.clear_flag_N();
        self.registers.clear_flag_H();
    }

    fn shift_right_arith(&mut self, src: SingleDataLoc) {
        let operand_value = self.read_single(&src);
        let bottom_bit = operand_value & 1;
        let output_value = (operand_value & 0x80) | (operand_value >> 1);
        self.write_single(&src, output_value);
        self.registers.flag_C_from_bool(bottom_bit > 0);
        self.registers.flag_Z_from_bool(output_value == 0);
        self.registers.clear_flag_N();
        self.registers.clear_flag_H();
    }

    fn shift_right_logic(&mut self, src: SingleDataLoc) {
        let operand_value = self.read_single(&src);
        let bottom_bit = operand_value & 1;
        let output_value = operand_value >> 1;
        self.write_single(&src, output_value);
        self.registers.flag_C_from_bool(bottom_bit > 0);
        self.registers.flag_Z_from_bool(output_value == 0);
        self.registers.clear_flag_N();
        self.registers.clear_flag_H();
    }

    fn swap(&mut self, src: SingleDataLoc) {
        let operand_value = self.read_single(&src);
        let output_value = ((operand_value & 0xF) << 4) | ((operand_value >> 4) & 0xF);
        self.write_single(&src, output_value);
        self.registers.clear_flag_N();
        self.registers.clear_flag_H();
        self.registers.clear_flag_C();
        self.registers.flag_Z_from_bool(output_value == 0);
    }

    fn handle_no_params_shifts(&mut self, opcode: u8) {
        let src_reg_i = opcode as u8 & 0x7;
        let src_reg: SingleDataLoc = SingleDataLoc::from((src_reg_i, None));
        let operand_value = self.read_single(&src_reg);
        if (opcode >> 3) & 0x7 == 0x0 {
            if DEBUG {
                println!("> RLC {src_reg:?}");
            }
            self.rotate_left_circular(src_reg);
        } else if (opcode >> 3) & 0x7 == 0x1 {
            if DEBUG {
                println!("> RRC {src_reg:?}");
            }
            self.rotate_right_circular(src_reg);
        } else if (opcode >> 3) & 0x7 == 0x2 {
            if DEBUG {
                println!("> RL {src_reg:?}");
            }
            self.rotate_left(src_reg);
        } else if (opcode >> 3) & 0x7 == 0x3 {
            if DEBUG {
                println!("> RR {src_reg:?}");
            }
            self.rotate_right(src_reg);
        } else if (opcode >> 3) & 0x7 == 0x4 {
            if DEBUG {
                println!("> SLA {src_reg:?}");
            }
            self.shift_left_arith(src_reg);
        } else if (opcode >> 3) & 0x7 == 0x5 {
            if DEBUG {
                println!("> SRA {src_reg:?}");
            }
            self.shift_right_arith(src_reg);
        } else if (opcode >> 3) & 0x7 == 0x6 {
            if DEBUG {
                println!("> SWAP {src_reg:?}");
            }
            self.swap(src_reg);
        } else if (opcode >> 3) & 0x7 == 0x7 {
            if DEBUG {
                println!("> SRL {src_reg:?}");
            }
            self.shift_right_logic(src_reg);
        }
    }

    fn handle_bit_test(&mut self, opcode: u8) {
        let src_reg_i = opcode as u8 & 0x7;
        let src_reg: SingleDataLoc = SingleDataLoc::from((src_reg_i, None));
        let operand_value = self.read_single(&src_reg);
        let bit_n = (opcode >> 3) & 0x7;
        if DEBUG{
            println!("> BIT {bit_n}, {src_reg:?}")
        }
        self.registers.flag_Z_from_bool((operand_value >> bit_n) & 1 == 0);
        self.registers.clear_flag_N();
        self.registers.set_flag_H();
    }

    fn handle_bit_clear(&mut self, opcode: u8) {
        let src_reg_i = opcode as u8 & 0x7;
        let src_reg: SingleDataLoc = SingleDataLoc::from((src_reg_i, None));
        let operand_value = self.read_single(&src_reg);
        let bit_n = (opcode >> 3) & 0x7;
        if DEBUG{
            println!("> RES {bit_n}, {src_reg:?}")
        }
        let output = operand_value & (0xFF ^ (1 << bit_n));
        self.write_single(&src_reg, output);
    }

    fn handle_bit_set(&mut self, opcode: u8) {
        let src_reg_i = opcode as u8 & 0x7;
        let src_reg: SingleDataLoc = SingleDataLoc::from((src_reg_i, None));
        let operand_value = self.read_single(&src_reg);
        let bit_n = (opcode >> 3) & 0x7;
        if DEBUG{
            println!("> SET {bit_n}, {src_reg:?}")
        }
        let output = operand_value | (1 << bit_n);
        self.write_single(&src_reg, output);
    }

    fn decimal_adjust_acc(&mut self) {
        let mut output_value = self.registers.A;
        if self.registers.read_flag_N() {
            if self.registers.read_flag_C() {
                output_value = output_value.wrapping_sub(0x60);
            }
            if self.registers.read_flag_H() {
                output_value = output_value.wrapping_sub(0x6);
            }
        } else {
            if self.registers.read_flag_C() || self.registers.A > 0x99 {
                output_value = output_value.wrapping_add(0x60);
            }
            if self.registers.read_flag_H() || (self.registers.A & 0x0F) > 0x09 {
                output_value = output_value.wrapping_add(0x06);
            }
        }
        self.registers.A = output_value;
        self.registers.flag_Z_from_bool(output_value == 0);
        self.registers.clear_flag_H();
    }

    fn complement_acc(&mut self) {
        self.registers.A ^= 0xFF;
        self.registers.set_flag_H();
        self.registers.set_flag_N();
    }

    fn set_carry_flag(&mut self) {
        self.registers.set_flag_C();
        self.registers.clear_flag_H();
        self.registers.clear_flag_N();
    }

    fn complement_carry_flag(&mut self) {
        self.registers.flag_C_from_bool(!self.registers.read_flag_C());
        self.registers.clear_flag_H();
        self.registers.clear_flag_N();
    }

    fn handle_accumulator_misc(&mut self, opcode: u16) {
        if (opcode >> 3) & 0x3 == 0 {
            if DEBUG {
                println!("> DAA");
            }
            self.decimal_adjust_acc();
        } else if (opcode >> 3) & 0x3 == 1 {
            if DEBUG {
                println!("> CPL");
            }
            self.complement_acc();
        } else if (opcode >> 3) & 0x3 == 2 {
            if DEBUG {
                println!("> SCF");
            }
            self.set_carry_flag();
        } else if (opcode >> 3) & 0x3 == 3 {
            if DEBUG {
                println!("> CCF");
            }
            self.complement_carry_flag();
        }
    }
}

impl std::fmt::Display for CPU {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut flags = ['-', '-', '-', '-'];
        if self.registers.read_flag_Z() {
            flags[0] = 'Z';
        }
        if self.registers.read_flag_N() {
            flags[1] = 'N';
        }
        if self.registers.read_flag_H() {
            flags[2] = 'H';
        }
        if self.registers.read_flag_C() {
            flags[3] = 'C';
        }
        write!(f, "AF: {:04X}, BC: {:04X}, DE: {:04X}, HL: {:04X}, SP: {:04X}, PC: {:04X}, F: {} | IME: {} | T: {} | LCDC: {:02X} | STAT: {:02X} | LY: {:02X}", 
         self.registers.AF(), self.registers.BC(), self.registers.DE(), self.registers.HL(), 
         self.registers.SP, self.registers.PC(), String::from_iter(flags), self.master_interrupt_enable as u8, 
         self.clock, self.memory.read(0xFF40), self.memory.read(0xFF41), self.memory.read(0xFF44))
    }
}