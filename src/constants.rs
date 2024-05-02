use std::ops::RangeInclusive;

pub const GB_ROM_BANK_SIZE: usize = 16 * 1024;
pub const GB_INTERNAL_RAM_SIZE: usize = 8 * 1024;
pub const GB_VRAM_SIZE: usize = 8 * 1024;
pub const OAM_SIZE: usize = 160;
pub const CARTRIDGE_RAM_SIZE: usize = 8 * 1024;

pub const SCREEN_HEIGHT: usize = 144;
pub const SCREEN_WIDTH: usize = 160;
pub const NUM_DOTS_PER_LINE: u16 = 456;
pub const NUM_SCAN_LINES: u8 = 154;

pub const LCDC_BG_WIN_DISPLAY_BIT: u8 = 0;
pub const LCDC_OBJ_ENABLE_BIT: u8 = 1;
pub const LCDC_OBJ_SIZE_BIT: u8 = 2;
pub const LCDC_BG_TILE_MAP_BIT: u8 = 3;
pub const LCDC_BG_WINDOW_TILE_DATA_AREA_BIT: u8 = 4;
pub const LCDC_WINDOW_ENABLE_BIT: u8 = 5;
pub const LCDC_WINDOW_TILE_MAP_BIT: u8 = 6;
pub const LCDC_PPU_ENABLE_BIT: u8 = 7;

pub const LCDC_ADDR: u16 = 0xFF40;
pub const SCX_ADDR: u16 = 0xFF43;
pub const SCY_ADDR: u16 = 0xFF42;
pub const WX_ADDR: u16 = 0xFF4B;
pub const WY_ADDR: u16 = 0xFF4A;

pub const BGP_ADDR: u16 = 0xFF47;
pub const OBP0_ADDR: u16 = 0xFF48;
pub const OBP1_ADDR: u16 = 0xFF49;

pub const LCDY_ADDR: u16 = 0xFF44;
pub const STAT_ADDR: u16 = 0xFF41;
pub const LYC_ADDR: u16 = 0xFF45;

pub const SB_ADDR: u16 = 0xFF01;
pub const SC_ADDR: u16 = 0xFF02;

pub const IF_ADDR: u16 = 0xFF0F;
pub const IE_ADDR: u16 = 0xFFFF;

pub const JOYP_ADDR: u16 = 0xFF00;

pub const DIV_ADDR: u16 = 0xFF04;
pub const TIMA_ADDR: u16 = 0xFF05;
pub const TMA_ADDR: u16 = 0xFF06;
pub const TAC_ADDR: u16 = 0xFF07;

pub const DMA_ADDR: u16 = 0xFF46;

pub const CLOCK_FREQ_HZ: u32 = 4194304;

pub const NR52_ADDR: u16 = 0xFF26;
pub const NR51_ADDR: u16 = 0xFF25;
pub const NR50_ADDR: u16 = 0xFF24;

pub const NR10_ADDR: u16 = 0xFF10;
pub const NR11_ADDR: u16 = 0xFF11;
pub const NR12_ADDR: u16 = 0xFF12;
pub const NR13_ADDR: u16 = 0xFF13;
pub const NR14_ADDR: u16 = 0xFF14;

pub const NR21_ADDR: u16 = 0xFF16;
pub const NR22_ADDR: u16 = 0xFF17;
pub const NR23_ADDR: u16 = 0xFF18;
pub const NR24_ADDR: u16 = 0xFF19;

pub const NR30_ADDR: u16 = 0xFF1A;
pub const NR31_ADDR: u16 = 0xFF1B;
pub const NR32_ADDR: u16 = 0xFF1C;
pub const NR33_ADDR: u16 = 0xFF1D;
pub const NR34_ADDR: u16 = 0xFF1E;

pub const WAVE_RANGE_START: u16 = 0xFF30;
pub const WAVE_RANGE_ADDR: RangeInclusive<u16> = WAVE_RANGE_START..=0xFF3F;
pub const NR41_ADDR: u16 = 0xFF20;
pub const NR42_ADDR: u16 = 0xFF21;
pub const NR43_ADDR: u16 = 0xFF22;
pub const NR44_ADDR: u16 = 0xFF23;

pub const APU_ENABLE_BIT: u8 = 7;
pub const APU_CH1_ON_BIT: u8 = 0;
pub const APU_CH2_ON_BIT: u8 = 1;
pub const APU_CH3_ON_BIT: u8 = 2;
pub const APU_CH4_ON_BIT: u8 = 3;

pub const APU_CH1_PAN_RIGHT_BIT: u8 = 0;
pub const APU_CH2_PAN_RIGHT_BIT: u8 = 1;
pub const APU_CH3_PAN_RIGHT_BIT: u8 = 2;
pub const APU_CH4_PAN_RIGHT_BIT: u8 = 3;

pub const APU_CH1_PAN_LEFT_BIT: u8 = 4;
pub const APU_CH2_PAN_LEFT_BIT: u8 = 5;
pub const APU_CH3_PAN_LEFT_BIT: u8 = 6;
pub const APU_CH4_PAN_LEFT_BIT: u8 = 7;

pub const AUDIO_BUFFER_NUM_SAMPLES: usize = 512;
pub const TARGET_SAMPLE_RATE: usize = 44100;

pub const HPF_CAPACITOR_CHARGE: f32 = 0.996;