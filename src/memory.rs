#![allow(non_camel_case_types)]
use crate::constants::*;
use crate::interrupt::Interrupt;

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

impl std::convert::From<u8> for Cartridge {
    fn from(value: u8) -> Self {
        match value {
            0x00 => Cartridge::RomOnly,
            0x01 => Cartridge::MBC1,
            0x03 => Cartridge::MBC1_RAM_BATTERY,
            0x05 => Cartridge::MBC2,
            0x11 => Cartridge::MBC3,
            0x13 => Cartridge::MBC3_RAM_BATTERY,
            0x19 => Cartridge::MBC5,
            0x20 => Cartridge::MBC6,
            _ => panic!("Unexpected value for Cartrigde '{value}'"),
        }
    }
}

pub struct AddressSpace {
    pub raw_game_rom: Vec<u8>,
    pub rom_bank: Vec<u8>,
    active_rom_bank: Vec<u8>,
    vram: [u8; GB_VRAM_SIZE],
    ram_bank: Vec<u8>,
    internal_ram: [u8; GB_INTERNAL_RAM_SIZE],
    oam: [u8; OAM_SIZE],
    empty_io: [u8; 96],
    standard_io: [u8; 76],
    empty_io2: [u8; 52],
    hram: [u8; 127],
    interrupt_enable: [u8; 1],
    dma_start_address: i32,
    dma_clock_t: u16,
    joypad_state: u8,
    oam_writeable: bool,
    vram_writeable: bool,
    internal_div: u16,
    past_tick_tima_enabled: bool,
    clock: u64,
    rom_select_register: u8,
    ram_select_register: u8,
    bank_mode_register: u8,
    external_ram_enable: bool,
    num_rom_banks: usize,
    num_ram_banks: usize,
}

impl AddressSpace {
    pub fn new() -> AddressSpace {
        AddressSpace {
            raw_game_rom: Vec::new(),
            rom_bank: Vec::new(),
            active_rom_bank: Vec::new(),
            vram: [0; GB_VRAM_SIZE],
            ram_bank: Vec::new(),
            internal_ram: [0; GB_INTERNAL_RAM_SIZE],
            oam: [0; OAM_SIZE],
            empty_io: [0; 96],
            standard_io: [0; 76],
            empty_io2: [0; 52],
            hram: [0; 127],
            interrupt_enable: [0; 1],
            dma_start_address: -1,
            dma_clock_t: 0,
            joypad_state: 0xFF,
            oam_writeable: false,
            vram_writeable: false,
            internal_div: 0,
            past_tick_tima_enabled: false,
            clock: 0,
            rom_select_register: 1,
            ram_select_register: 0,
            bank_mode_register: 0,
            external_ram_enable: false,
            num_rom_banks: 0,
            num_ram_banks: 0,
            
        }
    }

    pub fn request_interrupt(&mut self, interrupt: Interrupt) {
        let interrupt_mask = 1 << interrupt as usize;
        let mut interrupt_flags = self.read(IF_ADDR);
        interrupt_flags |= interrupt_mask;
        self.write(IF_ADDR, interrupt_flags);
    }

    pub fn load_rom(&mut self, rom_bytes: Vec<u8>) -> Result<(), String> {
        // let cartridge_type: Cartridge = Cartridge::from(rom_bytes[0x147]);
        // if cartridge_type != Cartridge::RomOnly {
        //     panic!("Only ROM only cartridges are supported, found {cartridge_type:?}");
        // }
        // match rom_bytes.len() {
        //     x if x > 0x8000 => panic!("ROM size too large"),
        //     x if x < 0x8000 => panic!("ROM size too small"),
        //     _ => (),

        // }
        self.num_rom_banks = 2 << rom_bytes[0x148];
        let rom_size = self.num_rom_banks * 16;
        println!("Rom with {} banks, total {rom_size} KB", self.num_rom_banks);
        if rom_size > 1024 {
            unimplemented!("Alternate RAM wiring!")
        }

        let ram_banks_code = rom_bytes[0x149];
        self.num_ram_banks = match ram_banks_code {
            0x00 => 0,
            0x01 => panic!("Unused ram code found"),
            0x02 => 1,
            0x03 => 4,
            0x04 => 16,
            0x05 => 8,
            _ => unreachable!("Invalid ram bank code found: {ram_banks_code}"),
        };
        let ram_size = self.num_ram_banks * CARTRIDGE_RAM_SIZE;
        println!("Ram with {} banks, total {ram_size} KB", self.num_ram_banks);
        
        self.ram_bank.resize(ram_size, 0);

        self.rom_bank.extend_from_slice(&rom_bytes[..GB_ROM_BANK_SIZE]);
        self.active_rom_bank.extend_from_slice(&rom_bytes[GB_ROM_BANK_SIZE..self.num_rom_banks*GB_ROM_BANK_SIZE]);
        self.raw_game_rom = rom_bytes.clone();

        if let Ok(title) = String::from_utf8(rom_bytes[0x134..0x144].to_vec()) {
            println!("Loading '{title}'");
        } else {
            println!("Couldn't load title.");
        }
        Ok(())
    }

    pub fn read(&self, index: u16) -> u8 {
        let value = match index {
            0..=0x3FFF => {
                if self.num_rom_banks <= 32 || self.bank_mode_register == 0 {
                    return self.rom_bank[index as usize];
                }
                let bank_number = (self.ram_select_register << 5) as usize;
                let bank_offset = (bank_number - 1) * 0x4000;
                self.active_rom_bank[bank_offset + index as usize]
            },
            0x4000..=0x7FFF => {
                let bank_number;
                if self.num_rom_banks <= 32 {
                    bank_number = self.rom_select_register as usize;
                    if bank_number == 0 {
                        return self.rom_bank[index as usize - 0x4000]
                    }
                } else {
                    bank_number = (self.ram_select_register << 5) as usize | self.rom_select_register as usize;
                }
                let bank_offset = (bank_number - 1) * 0x4000;
                self.active_rom_bank[bank_offset + index as usize - 0x4000]
            },
            0x8000..=0x9FFF => if self.vram_writeable {
                self.vram[index as usize - 0x8000]
            } else {
                0xFF
            },
            0xA000..=0xBFFF => {
                if !self.external_ram_enable {
                    return 0xFF
                }
                if self.bank_mode_register == 0 {
                    self.ram_bank[index as usize - 0xA000]
                } else {
                    let bank_number = self.ram_select_register as usize;
                    let bank_offset = bank_number * 0x2000;
                    self.ram_bank[bank_offset + index as usize - 0xA000]
                }
            },
            0xC000..=0xDFFF => self.internal_ram[index as usize - 0xC000],
            0xE000..=0xFDFF => self.internal_ram[index as usize - 0xE000],
            0xFE00..=0xFE9F => if self.oam_writeable {
                self.oam[index as usize - 0xFE00]
            } else {
                0xFF
            },
            0xFEA0..=0xFEFF => self.empty_io[index as usize - 0xFEA0],
            JOYP_ADDR => self.joypad_return(),
            idx @ 0xFF01..=0xFF4B => {
                if idx == DIV_ADDR {
                    (self.internal_div >> 8) as u8
                } else {
                    self.standard_io[index as usize - 0xFF00]
                }
            },
            0xFF4C..=0xFF7F => self.empty_io2[index as usize - 0xFF4C],
            0xFF80..=0xFFFE => self.hram[index as usize - 0xFF80],
            IE_ADDR => self.interrupt_enable[index as usize - 0xFFFF],
        };
        value
    }

    pub fn write(&mut self, index: u16, value: u8) {
        match index {
            // 0..=0x7FFF => println!("Tried to write into {:02X} which is not writeable", index),
            0..=0x1FFF => {
                if value & 0xF == 0xA {
                    self.external_ram_enable = true;
                } else {
                    self.external_ram_enable = false;
                }
            },
            0x2000..=0x3FFF => {
                // self.selected_rom_bank = value & (self.num_rom_banks as u8 - 1); // TODO: implement when checking selected rom
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
            0x8000..=0x9FFF => if self.vram_writeable {
                self.vram[index as usize - 0x8000] = value
            },
            0xA000..=0xBFFF => {
                if !self.external_ram_enable || self.ram_bank.len() == 0 {
                    return
                }
                if self.bank_mode_register == 0 {
                    self.ram_bank[index as usize - 0xA000] = value;
                } else {
                    let bank_number = self.ram_select_register as usize;
                    let bank_offset = bank_number * 0x2000;
                    self.ram_bank[bank_offset + index as usize - 0xA000] = value
                }
            }
            0xC000..=0xDFFF => self.internal_ram[index as usize - 0xC000] = value,
            0xE000..=0xFDFF => self.internal_ram[index as usize - 0xE000] = value,
            0xFE00..=0xFE9F => {
                if self.oam_writeable {
                    self.oam[index as usize - 0xFE00] = value
                }
            }
            0xFEA0..=0xFEFF => self.empty_io[index as usize - 0xFEA0] = value,
            idx @ 0xFF00..=0xFF4B => 
            {
                if idx == DMA_ADDR {
                    if self.dma_start_address >= 0 {
                        ();
                    } else {
                        self.dma_start_address = (0x100 * value as u16) as i32;
                    }
                } else if idx == JOYP_ADDR {
                    self.standard_io[0] = (value & 0xF0) | (self.standard_io[0] & 0xF)
                } else if idx == STAT_ADDR {
                    self.standard_io[index as usize - 0xFF00] = (value & 0x78) | (self.standard_io[index as usize - 0xFF00] & 0x07)
                } else if idx == LCDY_ADDR {
                    ()
                } else if idx == SCX_ADDR {
                    self.standard_io[index as usize - 0xFF00] = value;
                } else if idx == LCDC_ADDR {
                    self.standard_io[index as usize - 0xFF00] = value;
                } else if idx == DIV_ADDR {
                    self.internal_div = 0
                } else if idx == SB_ADDR {
                    self.standard_io[index as usize - 0xFF00] = value
                } else {
                    self.standard_io[index as usize - 0xFF00] = value
                }
            }
            0xFF4C..=0xFF7F => self.empty_io2[index as usize - 0xFF4C] = value,
            0xFF80..=0xFFFE => self.hram[index as usize - 0xFF80] = value,
            IE_ADDR => self.interrupt_enable[index as usize - 0xFFFF] = value,
        };
    }

    pub fn read_sprite(&self, sprite_index: u8) -> &[u8] {
        let idx = sprite_index as usize;
        return &self.oam[idx..idx+4]
    }

    fn joypad_return(&self) -> u8 {
        let selection = (self.standard_io[0] >> 4) & 0x3;
        if selection == 0x3 { // no selection, return no key presses
            return (selection << 4) | 0xF
        } else {
            let mut state = 0xFF;
            if selection & 1 == 0 { // select d-pad
                state &= self.joypad_state & 0xF;
            }

            if (selection >> 1) & 1 == 0 { // select buttons
                state &= (self.joypad_state >> 4) & 0xF;
            }
            state
        }
    }

    pub fn _lock_vram(&mut self) {
        self.vram_writeable = false;
    }

    pub fn unlock_vram(&mut self) {
        self.vram_writeable = true;
    }

    pub fn lock_oam(&mut self) {
        self.oam_writeable = false;
    }

    pub fn unlock_oam(&mut self) {
        self.oam_writeable = true;
    }

    pub fn ppu_write_stat(&mut self, value: u8) {
        self.standard_io[STAT_ADDR as usize - 0xFF00] = (value & 0x7) | (self.standard_io[STAT_ADDR as usize - 0xFF00] & 0x78);
    }

    pub fn ppu_write_LY_update_STAT(&mut self, ly_value: u8) {
        self.standard_io[LCDY_ADDR as usize - 0xFF00] = ly_value;
        let lyc = self.standard_io[LYC_ADDR as usize - 0xFF00];
        
        let mut stat_value = self.standard_io[STAT_ADDR as usize - 0xFF00];
        if ly_value == lyc {
            stat_value |= 1 << 2;
        } 
        else {
            stat_value &= 0xFF ^ (1 << 2);
        }
        self.standard_io[STAT_ADDR as usize - 0xFF00] = stat_value;
    }

    pub fn _ppu_write_LY(&mut self, ly_value: u8) {
        self.standard_io[LCDY_ADDR as usize - 0xFF00] = ly_value;
    }

    pub fn joypad_write(&mut self, state: u8) {
        self.joypad_state = state;
    }

    fn dma_single_tick(&mut self) {
        if self.dma_start_address < 0 {
            return
        }
        let oam_index = (self.dma_clock_t) as usize;
        let mem_address = (self.dma_start_address as u16) + self.dma_clock_t;
        self.oam[oam_index] = self.read(mem_address);
        // println!("DMA step, clock {} -> offset: {}, base: ${:04X} -> oam[${oam_index:02X}] = self.read(${mem_address:04X}) = ${:02X}", self.dma_clock_t, self.dma_clock_t / 4, self.dma_start_address, self.oam[oam_index]);
        self.dma_clock_t += 1;
        if self.dma_clock_t >= 0xA0 {
            self.dma_clock_t = 0;
            self.dma_start_address = -1;
        }
    }

    fn increment_tima(&mut self) {
        let mut tima = self.standard_io[TIMA_ADDR as usize - 0xFF00];
        tima = match tima.overflowing_add(1) {
            (_, true) => {
                self.request_interrupt(Interrupt::Timer);
                self.standard_io[TMA_ADDR as usize - 0xFF00]
            },
            (value, false) => value,
        };
        self.standard_io[TIMA_ADDR as usize - 0xFF00] = tima;
    }

    fn increment_div(&mut self) -> bool{
        self.internal_div =  self.internal_div.wrapping_add(1);
        let tac_reg = self.standard_io[TAC_ADDR as usize - 0xFF00];
        let bit_selected = match tac_reg & 0x3 {
            0 => 9,
            1 => 3,
            2 => 5,
            3 => 7,
            _ => unreachable!(),
        };
        let bit_set = (self.internal_div >> bit_selected) & 1 == 1;
        let timer_enabled = (tac_reg >> 2) & 1 == 1;
        timer_enabled & bit_set

    }

    pub fn tick(&mut self, nticks: u8) {
        for _ in 0..nticks {
            self.dma_single_tick();
            let tima_enabled = self.increment_div();
            if self.past_tick_tima_enabled & !tima_enabled {
                self.increment_tima();
            }
            self.past_tick_tima_enabled = tima_enabled;
            self.clock += 1;
        }
        
    }
}