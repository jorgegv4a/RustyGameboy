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
    fn save_persistent_state(&self) -> Vec<u8>;
    fn load_persistent_state(&mut self, state: Vec<u8>);
    fn cartridge_type(&self) -> Option<Cartridge>;
    fn tick(&mut self, nticks: u8);
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

    fn save_persistent_state(&self) -> Vec<u8> {
        vec![]
    }

    fn load_persistent_state(&mut self, state: Vec<u8>) {}

    fn cartridge_type(&self) -> Option<Cartridge> {
        None
    }

    fn tick(&mut self, nticks: u8) {}
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
        match index {
            0..=0x7FFF => return self.rom[index as usize],
            _ => unreachable!("Invalid access to ROM only cartridge at index {index}"),
        }
    }

    fn write(&mut self, index: u16, value: u8) {}

    fn save_persistent_state(&self) -> Vec<u8> {
        vec![]
    }

    fn load_persistent_state(&mut self, state: Vec<u8>) {}

    fn cartridge_type(&self) -> Option<Cartridge> {
        Some(Cartridge::RomOnly)
    }

    fn tick(&mut self, nticks: u8) {}
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
        if cartridge_type != Cartridge::MBC1 && cartridge_type != Cartridge::MBC1_RAM_BATTERY {
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

    fn save_persistent_state(&self) -> Vec<u8> {
        self.ram.clone()
    }

    fn load_persistent_state(&mut self, state: Vec<u8>) {
        self.ram = state;
    }
    
    fn cartridge_type(&self) -> Option<Cartridge> {
        Some(Cartridge::MBC1)
    }

    fn tick(&mut self, nticks: u8) {}
}


#[derive(Debug, Clone, Copy)]
pub struct RTCreg {
    RTCS: u8,
    RTCM: u8,
    RTCH: u8,
    RTCDL: u8,
    RTCDH: u8,
}

impl RTCreg {
    fn new() -> RTCreg {
        RTCreg {
            RTCS: 0,
            RTCM: 0,
            RTCH: 0,
            RTCDL: 0,
            RTCDH: 0,
        }
    }
}

#[derive(Debug)]
pub struct MBC3 {
    rom: Vec<u8>,
    ram: Vec<u8>,
    rom_select_register: u8,
    ram_select_register: u8,
    bank_mode_register: u8,
    external_ram_enable: bool,
    latch_clock: u8,
    rtc: RTCreg,
    latched_rtc: Option<RTCreg>,
    ticks_since_last_second: u32,
    rtc_halted: bool,
    
}

impl Addressable for MBC3 {
    fn new(game_bytes: Vec<u8>) -> Self {
        let cartridge_type: Cartridge = Cartridge::from(game_bytes[0x147]);
        if cartridge_type != Cartridge::MBC3 && cartridge_type != Cartridge::MBC3_RAM_BATTERY  {
            panic!("MBC3 cartridge expected, found {cartridge_type:?}");
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
        
        MBC3 {
            rom: game_bytes[..num_rom_banks * GB_ROM_BANK_SIZE].to_vec(),
            ram: vec![0; ram_size],
            rom_select_register: 1,
            ram_select_register: 0,
            bank_mode_register: 0,
            external_ram_enable: false,
            latch_clock: 255,
            rtc: RTCreg::new(),
            latched_rtc: None,
            ticks_since_last_second: 0,
            rtc_halted: false,
        }
    }

    fn read(&self, index: u16) -> u8 {
        let value = match index {
            0..=0x3FFF => {
                self.rom[index as usize]
            },
            0x4000..=0x7FFF => {
                let bank_number = self.rom_select_register as usize;
                let bank_offset = bank_number * 0x4000;
                self.rom[bank_offset + index as usize - 0x4000]
            },
            0xA000..=0xBFFF => {
                if !self.external_ram_enable {
                    return 0xFF
                }

                match self.ram_select_register {
                    0..=4 => {
                        let bank_number;
                        if self.bank_mode_register == 0 {
                            bank_number = 0;
                        } else {
                            bank_number = self.ram_select_register as usize;
                        }
                        let bank_offset = bank_number * 0x2000;
                        self.ram[bank_offset + index as usize - 0xA000]
                    }
                    0x8 => {
                        self.latched_rtc.unwrap().RTCS
                    }
                    0x9 => {
                        self.latched_rtc.unwrap().RTCH
                    }
                    0xA => {
                        self.latched_rtc.unwrap().RTCM
                    }
                    0xB => {
                        self.latched_rtc.unwrap().RTCDL
                    }
                    0xC => {
                        self.latched_rtc.unwrap().RTCDH
                    }
                    _ => 0xFF,
                }
            },
            _ => unreachable!("Invalid access to MBC3 cartridge at index {index}"),
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
                if value == 0 {
                    self.rom_select_register = 1;
                } else {
                    self.rom_select_register = value & 0x7F;
                }
            }
            0x4000..=0x5FFF => {
                match value {
                    0..=0x3 => self.ram_select_register = value & 0x03,
                    0x8..=0xC => {
                        self.ram_select_register = value;
                    },
                    _ => ()
                }
            }
            0x6000..=0x7FFF => {
                if self.latch_clock == 0 && value == 1 {
                    self.latched_rtc = Some(self.rtc);
                }
                self.latch_clock = value;
            }
            0xA000..=0xBFFF => {
                if !self.external_ram_enable || self.ram.len() == 0 {
                    return
                }

                match self.ram_select_register {
                    0..=4 => {
                        let bank_number;
                        if self.bank_mode_register == 0 {
                            bank_number = 0;
                        } else {
                            bank_number = self.ram_select_register as usize;
                        }
                        let bank_offset = bank_number * 0x2000;
                        self.ram[bank_offset + index as usize - 0xA000] = value
                    }
                    0x8 => {
                        self.rtc.RTCS = value & 0x3F;
                        self.latched_rtc.unwrap().RTCS = value & 0x3F;
                        self.ticks_since_last_second = 0;
                    }
                    0x9 => {
                        self.rtc.RTCH = value & 0x3F;
                        self.latched_rtc.unwrap().RTCH = value & 0x3F;
                    }
                    0xA => {
                        self.rtc.RTCM = value & 0x1F;
                        self.latched_rtc.unwrap().RTCM = value & 0x1F;
                    }
                    0xB => {
                        self.rtc.RTCDL = value & 0xFF;
                        self.latched_rtc.unwrap().RTCDL = value & 0xFF;
                    }
                    0xC => {
                        self.rtc.RTCDH = value & 0xC1;
                        self.latched_rtc.unwrap().RTCDH = value & 0xC1;
                        if (self.rtc.RTCDH >> 6) & 1 == 1 {
                            self.rtc_halted = true;
                        } else {
                            self.rtc_halted = false;
                        }
                    }
                    _ => unreachable!("Invalid value for self.ram_select_register={}", self.ram_select_register),
                }                    
            }
            _ => unreachable!("Invalid access to MBC3 cartridge at index {index}"),
        };
    }

    fn save_persistent_state(&self) -> Vec<u8> {
        self.ram.clone()
    }

    fn load_persistent_state(&mut self, state: Vec<u8>) {
        self.ram = state;
    }

    fn cartridge_type(&self) -> Option<Cartridge> {
        Some(Cartridge::MBC3)
    }

    fn tick(&mut self, nticks: u8) {
        if self.rtc_halted {
            return;
        }
        self.ticks_since_last_second += nticks as u32;
        if self.ticks_since_last_second >= CLOCK_FREQ_HZ {
            self.ticks_since_last_second -= CLOCK_FREQ_HZ;
            self.rtc.RTCS += 1;

            if self.rtc.RTCS == 60 {
                self.rtc.RTCS = 0;
                self.rtc.RTCM += 1;

                if self.rtc.RTCM == 60 {
                    self.rtc.RTCM = 0;
                    self.rtc.RTCH += 1;

                    if self.rtc.RTCH == 24 {
                        self.rtc.RTCH = 0;
                        let overflow;
                        (self.rtc.RTCDL, overflow) = self.rtc.RTCDL.overflowing_add(1);
                        
                        if overflow {
                            if self.rtc.RTCDH & 1 == 1 {
                                self.rtc.RTCDH &= 0x80;
                            } else {
                                self.rtc.RTCDH |= 1;
                            }
                        }
                    }
                }
            }
        }
    }
}