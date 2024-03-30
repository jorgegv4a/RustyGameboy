pub const GB_ROM_BANK_SIZE: usize = 16 * 1024;
pub const GB_INTERNAL_RAM_SIZE: usize = 8 * 1024;
pub const GB_VRAM_SIZE: usize = 8 * 1024;
pub const OAM_SIZE: usize = 160;
pub const CARTRIDGE_RAM_SIZE: usize = 8 * 1024;
pub const CARTRIDGE_ROM_ONLY: usize = 0x00;
pub const CARTRIDGE_MBC1: usize = 0x01;

pub const SCREEN_HEIGHT: usize = 144;
pub const SCREEN_WIDTH: usize = 160;
pub const NUM_DOTS_PER_LINE: usize = 456;
pub const NUM_SCAN_LINES: usize = 154;

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
pub const WX_ADDR: u16 = 0xFF4A;
pub const WY_ADDR: u16 = 0xFF4B;

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
