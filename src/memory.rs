#![allow(non_camel_case_types)]
use crate::constants::*;
use crate::interrupt::Interrupt;

#[derive(PartialEq)]
pub enum Cartridge {
    RomOnly,
    MBC1,
}

pub struct AddressSpace {
    pub raw_game_rom: Vec<u8>,
    pub rom_bank: [u8; GB_ROM_BANK_SIZE],
    active_rom_bank: [u8; GB_ROM_BANK_SIZE],
    vram: [u8; GB_VRAM_SIZE],
    ram_bank: [u8; GB_INTERNAL_RAM_SIZE],
    internal_ram: [u8; GB_INTERNAL_RAM_SIZE],
    oam: [u8; OAM_SIZE],
    empty_io: [u8; 96],
    standard_io: [u8; 76],
    empty_io2: [u8; 52],
    hram: [u8; 127],
    interrupt_enable: [u8; 1],
    dma_start_address: i32,
    dma_clock_t: u16,
}

impl AddressSpace {
    pub fn new() -> AddressSpace {
        AddressSpace {
            raw_game_rom: Vec::new(),
            rom_bank: [0; GB_ROM_BANK_SIZE],
            active_rom_bank: [0; GB_ROM_BANK_SIZE],
            vram: [0; GB_VRAM_SIZE],
            ram_bank: [0; GB_INTERNAL_RAM_SIZE],
            internal_ram: [0; GB_INTERNAL_RAM_SIZE],
            oam: [0; OAM_SIZE],
            empty_io: [0; 96],
            standard_io: [0; 76],
            empty_io2: [0; 52],
            hram: [0; 127],
            interrupt_enable: [0; 1],
            dma_start_address: -1,
            dma_clock_t: 0,
        }
    }

    pub fn request_interrupt(&mut self, interrupt: Interrupt) {
        let interrupt_mask = (1 << interrupt as usize);
        let mut interrupt_flags = self.read(IF_ADDR);
        interrupt_flags |= interrupt_mask;
        self.write(IF_ADDR, interrupt_flags);
    }

    pub fn load_rom(&mut self, rom_bytes: Vec<u8>, cartridge_type: Cartridge) -> Result<(), String> {
        if cartridge_type != Cartridge::RomOnly {
            panic!("Only ROM only cartridges are supported");
        }
        match rom_bytes.len() {
            x if x > 0x8000 => panic!("ROM size too large"),
            x if x < 0x8000 => panic!("ROM size too small"),
            _ => (),

        }
        // for i in 0..self.rom_bank.len() {
        //     self.rom_bank[i] = rom_bytes[i];
        // }
        // for i in 0..self.active_rom_bank.len() {
        //     self.active_rom_bank[i] = rom_bytes[self.rom_bank.len() + i];
        // }
        self.rom_bank.copy_from_slice(&rom_bytes[..GB_ROM_BANK_SIZE]);
        self.active_rom_bank.copy_from_slice(&rom_bytes[GB_ROM_BANK_SIZE..]);
        self.raw_game_rom = rom_bytes.clone();
        Ok(())
    }

    pub fn read(&self, index: u16) -> u8 {
        let value = match index {
            0..=0x3FFF => self.rom_bank[index as usize],
            0x4000..=0x7FFF => self.active_rom_bank[index as usize - 0x4000],
            0x8000..=0x9FFF => self.vram[index as usize - 0x8000],
            0xA000..=0xBFFF => self.ram_bank[index as usize - 0xA000],
            0xC000..=0xDFFF => self.internal_ram[index as usize - 0xC000],
            0xE000..=0xFDFF => self.internal_ram[index as usize - 0xE000],
            0xFE00..=0xFE9F => self.oam[index as usize - 0xFE00],
            0xFEA0..=0xFEFF => self.empty_io[index as usize - 0xFEA0],
            0xFF00 => 0xFF,
            // 0xFF01..=0xFF43 => self.standard_io[index as usize - 0xFF00],
            // 0xFF44 => 0x90,
            // 0xFF45..=0xFF4B => self.standard_io[index as usize - 0xFF00],
            0xFF01..=0xFF4B => self.standard_io[index as usize - 0xFF00],
            0xFF4C..=0xFF7F => self.empty_io2[index as usize - 0xFF4C],
            0xFF80..=0xFFFE => self.hram[index as usize - 0xFF80],
            0xFFFF => self.interrupt_enable[index as usize - 0xFFFF],
        };
        value
    }

    pub fn write(&mut self, index: u16, value: u8) {
        match index {
            0..=0x7FFF => println!("Tried to write into {:02X} which is not writeable", index),
            0x8000..=0x9FFF => self.vram[index as usize - 0x8000] = value,
            0xA000..=0xBFFF => self.ram_bank[index as usize - 0xA000] = value,
            0xC000..=0xDFFF => self.internal_ram[index as usize - 0xC000] = value,
            0xE000..=0xFDFF => self.internal_ram[index as usize - 0xE000] = value,
            0xFE00..=0xFE9F => self.oam[index as usize - 0xFE00] = value, // TODO: disable access except during H/V Blank
            0xFEA0..=0xFEFF => self.empty_io[index as usize - 0xFEA0] = value,
            idx @ 0xFF00..=0xFF4B => 
            {
                if idx == 0xFF46 {
                    self.dma_start_address = 0x100 * value as i32
                } else{
                    self.standard_io[index as usize - 0xFF00] = value
                }
            }
            0xFF4C..=0xFF7F => self.empty_io2[index as usize - 0xFF4C] = value,
            0xFF80..=0xFFFE => self.hram[index as usize - 0xFF80] = value,
            0xFFFF => self.interrupt_enable[index as usize - 0xFFFF] = value,
        };
    }

    fn single_tick(&mut self) {
        if self.dma_start_address < 0 {
            return
        }
        let oam_index = (self.dma_clock_t / 4) as usize;
        let mem_address = (self.dma_start_address as u16) + self.dma_clock_t / 4;
        self.oam[oam_index] = self.read(mem_address);
        if self.dma_clock_t >= 0xA0 {
            self.dma_clock_t = 0;
            self.dma_start_address = -1;
        }
    }

    pub fn tick(&mut self, nticks: u8) {
        for i in 0..nticks {
            self.single_tick();
        }
    }
}