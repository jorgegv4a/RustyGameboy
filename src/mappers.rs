#![allow(non_camel_case_types)]

use crate::constants::*;

#[derive(PartialEq, Debug)]
pub enum Cartridge {
    RomOnly = 0x00,
    MBC1 = 0x01,
    MBC1_RAM_BATTERY = 0x03,
    MBC2 = 0x05,
    MBC3 = 0x11,
    MBC3_RAM_BATTERY = 0x13,
    MBC5 = 0x19,
    MBC6 = 0x20,
}

pub trait Addressable {
    fn new(game_bytes: Vec<u8>) -> Self where Self: Sized;
    fn read(&self, index: u16) -> u8;
    fn write(&mut self, index: u16, value: u8);
    fn cartridge_type(&self) -> Option<Cartridge>;
}



#[derive(Debug)]
pub struct NoCartridge {
}

impl Addressable for NoCartridge {
    fn new(game_bytes: Vec<u8>) -> Self {
        NoCartridge {
        }
    }

    fn read(&self, index: u16) -> u8 {
        match index {
            0..=0x7FFF => 0xFF,
            _ => unreachable!("Invalid access to ROM only cartridge at index {index}"),
        }
    }

    fn write(&mut self, index: u16, value: u8) {}

    fn cartridge_type(&self) -> Option<Cartridge> {
        None
    }
}


#[derive(Debug)]
pub struct RomOnly {
    rom: Vec<u8>,
}

impl Addressable for RomOnly {
    fn new(game_bytes: Vec<u8>) -> Self {
        let cartridge_type: Cartridge = Cartridge::from(game_bytes[0x147]);
        if cartridge_type != Cartridge::RomOnly {
            panic!("RomOnly cartridge expected, found {cartridge_type:?}");
        }
        RomOnly {
            rom: game_bytes[..2*GB_ROM_BANK_SIZE].to_vec(),
        }
    }

    fn read(&self, index: u16) -> u8 {
        let value = match index {
            0..=0x7FFF => return self.rom[index as usize],
            _ => unreachable!("Invalid access to ROM only cartridge at index {index}"),
        };
    }

    fn write(&mut self, index: u16, value: u8) {}

    fn cartridge_type(&self) -> Option<Cartridge> {
        Some(Cartridge::RomOnly)
    }
}


#[derive(Debug)]
pub struct MBC1 {
    rom: Vec<u8>,
    ram: Vec<u8>,
    rom_select_register: u8,
    ram_select_register: u8,
    bank_mode_register: u8,
    external_ram_enable: bool,
    num_rom_banks: usize,
    num_ram_banks: usize,
}

impl Addressable for MBC1 {
    fn new(game_bytes: Vec<u8>) -> Self {
        let cartridge_type: Cartridge = Cartridge::from(game_bytes[0x147]);
        if cartridge_type != Cartridge::MBC1 {
            panic!("MBC1 cartridge expected, found {cartridge_type:?}");
        }

        let num_rom_banks = 2 << game_bytes[0x148];
        let rom_size = num_rom_banks * 16;
        println!("Rom with {num_rom_banks} banks, total {rom_size} KB");
        if rom_size > 1024 {
            unimplemented!("Alternate RAM wiring!")
        }

        let ram_banks_code = game_bytes[0x149];
        let num_ram_banks = match ram_banks_code {
            0x00 => 0,
            0x01 => panic!("Unused ram code found"),
            0x02 => 1,
            0x03 => 4,
            0x04 => 16,
            0x05 => 8,
            _ => unreachable!("Invalid ram bank code found: {ram_banks_code}"),
        };
        let ram_size = num_ram_banks * CARTRIDGE_RAM_SIZE;
        println!("Ram with {num_ram_banks} banks, total {ram_size} KB");
        
        MBC1 {
            rom: game_bytes[..num_rom_banks * GB_ROM_BANK_SIZE].to_vec(),
            ram: vec![0; ram_size],
            rom_select_register: 1,
            ram_select_register: 0,
            bank_mode_register: 0,
            external_ram_enable: false,
            num_rom_banks,
            num_ram_banks,
        }
    }

    fn read(&self, index: u16) -> u8 {
        let value = match index {
            0..=0x3FFF => {
                let bank_number;
                if self.num_rom_banks <= 32 || self.bank_mode_register == 0 {
                    bank_number = 0;
                } else {
                    bank_number = (self.ram_select_register << 5) as usize;
                }
                let bank_offset = bank_number * 0x4000;
                self.rom[bank_offset + index as usize]
            },
            0x4000..=0x7FFF => {
                let bank_number;
                if self.num_rom_banks <= 32 {
                    bank_number = self.rom_select_register as usize;
                } else {
                    bank_number = (self.ram_select_register << 5) as usize | self.rom_select_register as usize;
                }
                let bank_offset = bank_number * 0x4000;
                self.rom[bank_offset + index as usize - 0x4000]
            },
            0x8000..=0x9FFF => unreachable!("Invalid access to MBC1 cartridge at index {index}"),
            0xA000..=0xBFFF => {
                if !self.external_ram_enable {
                    return 0xFF
                }
                let bank_number;
                if self.bank_mode_register == 0 {
                    bank_number = 0;
                } else {
                    bank_number = self.ram_select_register as usize;
                }
                let bank_offset = bank_number * 0x2000;
                self.ram[bank_offset + index as usize - 0xA000]
            },
            _ => unreachable!("Invalid access to MBC1 cartridge at index {index}"),
        };
        value
    }

    fn write(&mut self, index: u16, value: u8) {
        match index {
            0..=0x1FFF => {
                if value & 0xF == 0xA {
                    self.external_ram_enable = true;
                } else {
                    self.external_ram_enable = false;
                }
            },
            0x2000..=0x3FFF => {
                if value & 0x1F == 0 {
                    self.rom_select_register = 1;
                } else {
                    self.rom_select_register = value & std::cmp::min(0x1F, self.num_rom_banks - 1) as u8;
                }
            }
            0x4000..=0x5FFF => {
                if self.num_ram_banks > 1 || self.num_rom_banks > 32 {
                    self.ram_select_register = value & 0x03;
                }
            }
            0x6000..=0x7FFF => {
                self.bank_mode_register = value & 0x01;
            }
            0x8000..=0x9FFF => unreachable!("Invalid access to MBC1 cartridge at index {index}"),
            0xA000..=0xBFFF => {
                if !self.external_ram_enable || self.ram.len() == 0 {
                    return
                }
                let bank_number;
                if self.bank_mode_register == 0 {
                    bank_number = 0;
                } else {
                    bank_number = self.ram_select_register as usize;
                }
                let bank_offset = bank_number * 0x2000;
                self.ram[bank_offset + index as usize - 0xA000] = value
            }
            _ => unreachable!("Invalid access to MBC1 cartridge at index {index}"),
        };
    }

    fn cartridge_type(&self) -> Option<Cartridge> {
        Some(Cartridge::MBC1)
    }
}