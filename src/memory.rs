#![allow(non_camel_case_types)]
use crate::constants::*;
use crate::interrupt::Interrupt;
use crate::mappers::{Addressable, Cartridge, NoCartridge, RomOnly, MBC1};

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
    vram: [u8; GB_VRAM_SIZE],
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
    mapper: Box<dyn Addressable>,
}

impl AddressSpace {
    pub fn new() -> AddressSpace {
        AddressSpace {
            vram: [0; GB_VRAM_SIZE],
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
            mapper: Box::new(NoCartridge::new(Vec::new())),
        }
    }

    pub fn request_interrupt(&mut self, interrupt: Interrupt) {
        let interrupt_mask = 1 << interrupt as usize;
        let mut interrupt_flags = self.read(IF_ADDR);
        interrupt_flags |= interrupt_mask;
        self.write(IF_ADDR, interrupt_flags);
    }

    pub fn load_rom(&mut self, game_bytes: Vec<u8>) -> Result<(), String> {
        let cartridge_type: Cartridge = Cartridge::from(game_bytes[0x147]);
        match cartridge_type {
            Cartridge::RomOnly => self.mapper = Box::new(RomOnly::new(game_bytes)),
            Cartridge::MBC1 => self.mapper = Box::new(MBC1::new(game_bytes)),
            // _ => unimplemented!("No mapper implemented for cartridge type {cartridge_type:?}"),
            _ => self.mapper = Box::new(MBC1::new(game_bytes)),
        };
        println!("Cartridge mapper '{cartridge_type:?}'");
        Ok(())
    }

    pub fn read(&self, index: u16) -> u8 {
        let value = match index {
            0..=0x7FFF => {
                self.mapper.read(index)
            },
            0x8000..=0x9FFF => if self.vram_writeable {
                self.vram[index as usize - 0x8000]
            } else {
                0xFF
            },
            0xA000..=0xBFFF => {
                self.mapper.read(index)
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
            0..=0x7FFF => {
                self.mapper.write(index, value)
            }
            0x8000..=0x9FFF => if self.vram_writeable {
                self.vram[index as usize - 0x8000] = value
            },
            0xA000..=0xBFFF => {
                self.mapper.write(index, value)
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