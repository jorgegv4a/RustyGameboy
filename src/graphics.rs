extern crate sdl2;

use sdl2::event::Event;
use sdl2::keyboard::Keycode;
use sdl2::pixels::Color;
use sdl2::VideoSubsystem;
use std::collections::HashSet;
// use std::ops::Add;
// use std::ptr::metadata;
use std::time::Duration;
use std::collections::HashMap;
use std::cmp::min;
use sdl2::rect::Point;

use rand::prelude::*;

use crate::constants::*;
use crate::interrupt::Interrupt;
use crate::memory::AddressSpace;

#[derive(Debug)]
pub enum PPUMode {
    HBlank,
    VBlank,
    OAMScan,
    Drawing,
}


pub struct PPU {
    dot: u16,
    ly: u8,
    // drawing_current_line: bool,
    mode: PPUMode,
    tick_i: u64,
    canvas: sdl2::render::Canvas<sdl2::video::Window>,
}

// let sdl_context = sdl2::init()?;
// let video_subsystem = sdl_context.video()?;

// let window = video_subsystem
//     .window("rust-sdl2 demo: Video", 800, 600)
//     .position_centered()
//     .opengl()
//     .build()
//     .map_err(|e| e.to_string())?;

impl PPU {
    pub fn new(sdl_subsystem: VideoSubsystem) -> PPU {
        let window = sdl_subsystem
            .window("rust-sdl2 demo: Video", SCREEN_WIDTH as u32, SCREEN_HEIGHT as u32)
            .position_centered()
            .opengl()
            .build()
            .map_err(|e| e.to_string()).unwrap();
        let canvas = window.into_canvas().build().map_err(|e| e.to_string()).unwrap();
        PPU {
            dot: 0,
            ly: 0,
            // drawing_current_line: false,
            mode: PPUMode::OAMScan,
            tick_i: 0,
            canvas,
        }
    }

    fn get_win_tile_map(&self, memory: &AddressSpace) -> bool {
        return (memory.read(LCDC_ADDR) >> LCDC_WINDOW_TILE_MAP_BIT) & 1 == 1;
    }

    fn get_bg_win_tile_data_zone(&self, memory: &AddressSpace) -> bool {
        return (memory.read(LCDC_ADDR) >> LCDC_BG_WINDOW_TILE_DATA_AREA_BIT) & 1 == 1;
    }

    fn get_bg_tile_map(&self, memory: &AddressSpace) -> bool {
        return (memory.read(LCDC_ADDR) >> LCDC_BG_TILE_MAP_BIT) & 1 == 1;
    }

    fn get_obj_size(&self, memory: &AddressSpace) -> bool {
        return (memory.read(LCDC_ADDR) >> LCDC_OBJ_SIZE_BIT) & 1 == 1;
    }

    fn get_ppu_enabled(&self, memory: &AddressSpace) -> bool {
        return (memory.read(LCDC_ADDR) >> LCDC_PPU_ENABLE_BIT) & 1 == 1;
    }

    fn get_obj_enabled(&self, memory: &AddressSpace) -> bool {
        return (memory.read(LCDC_ADDR) >> LCDC_OBJ_ENABLE_BIT) & 1 == 1;
    }

    fn scx(&self, memory: &AddressSpace) -> usize {
        return memory.read(SCX_ADDR) as usize;
    }

    fn scy(&self, memory: &AddressSpace) -> usize {
        return memory.read(SCY_ADDR) as usize;
    }

    fn wx(&self, memory: &AddressSpace) -> usize {
        return memory.read(WX_ADDR) as usize;
    }

    fn wy(&self, memory: &AddressSpace) -> usize {
        return memory.read(WY_ADDR) as usize;
    }

    fn single_tick(&mut self, memory: &mut AddressSpace) {
        if !self.get_ppu_enabled(memory) {
            return
        }
        // println!("PPU, dot: {}, ly: {}, mode: {:?}", self.dot, self.ly, self.mode);
        match self.mode {
            PPUMode::OAMScan => {
                if self.dot == 80 {
                    self.mode = PPUMode::Drawing;
                    memory.request_interrupt(Interrupt::LCD);
                }
            },
            PPUMode::Drawing => {
                if self.dot == 80 + 172 { 
                    self.mode = PPUMode::HBlank;
                    memory.request_interrupt(Interrupt::LCD);
                }
            },
            PPUMode::HBlank => {
                if self.dot == 456 { 
                    self.dot = 0; 
                    self.ly += 1; 
                    if self.ly >= 144 { 
                        self.mode = PPUMode::VBlank;
                        memory.request_interrupt(Interrupt::VBlank);
                        memory.request_interrupt(Interrupt::LCD);
                    } else { 
                        self.mode = PPUMode::OAMScan;
                        memory.request_interrupt(Interrupt::LCD);
                    };
                }
            },
            PPUMode::VBlank => {
                if self.dot == 456 { 
                    self.dot = 0; 
                    self.ly += 1; 
                }
                if self.ly == 154 { 
                    self.ly = 0; 
                    self.mode = PPUMode::OAMScan;
                    memory.request_interrupt(Interrupt::LCD);
                };
            },
        };
        self.dot += 1;

    self.set_ly(memory);

        self.show(memory);
        self.tick_i += 1
    }

    fn set_ly(&self, memory: &mut AddressSpace) {
        memory.write(LCDY_ADDR, self.ly);
        let lyc = memory.read(LYC_ADDR);
        
        let mut value = memory.read(STAT_ADDR);
        let interrupt_on_equal_lyc = (value >> 6) & 1 == 1;

        if self.ly == lyc {
            value = value | (1 << 2);
            if interrupt_on_equal_lyc {
                memory.request_interrupt(Interrupt::LCD);
            }
        } else {
            value = value | (0xFF ^ (1 << 2));
        }
        memory.write(STAT_ADDR, value);

    }

    pub fn tick(&mut self, nticks: u8, memory: &mut AddressSpace) {
        for i in 0..nticks {
            self.single_tick(memory);
        }
    }

    fn get_background_tile_map(&self, memory: &AddressSpace) -> ([[u16; 32]; 32], HashSet<u16>) {
        let mut tile_map = [[0u16; 32]; 32];
        let tile_map_base_address = if self.get_bg_tile_map(memory) { 0x9C00 } else { 0x9800 };
        let mut unique_tiles: HashSet<u16> = HashSet::new();
        for tile_map_idx in 0..1024 {
            let tile_x = tile_map_idx % 32;
            let tile_y = tile_map_idx / 32;
            let tile_offset = memory.read(tile_map_base_address + tile_map_idx) as u16;
            let tile_address;
            if self.get_bg_win_tile_data_zone(memory) {
                tile_address = 0x8000 + tile_offset * 16;
            } else {
                tile_address = (0x9000 + ((tile_offset as i32) - 128) * 16) as u16
            }
            tile_map[tile_y as usize][tile_x as usize] = tile_address;
            unique_tiles.insert(tile_address);
        }
        (tile_map, unique_tiles)
    }

    fn get_tile(&self, memory: &AddressSpace, tile_start_i: u16) -> [[u8; 8]; 8] {
        let mut tile = [[0u8; 8]; 8];
        for j in 0..8 {
            let tile_low_byte = memory.read(tile_start_i + 2 * j);
            let tile_high_byte = memory.read((tile_start_i + 2 * j) + 1);
            for i in 0..8 {
                let low_byte = ((tile_low_byte >> i) & 1);
                let high_byte = (((tile_high_byte >> i) & 1) << 1);
                let color_id = high_byte | low_byte;
                let color = (((color_id as f32) / 3f32) * 255f32) as u8;
                tile[j as usize][(7 - i) as usize] = 255 - color;
            }
        };
        tile
    }

    fn show(&mut self, memory: &AddressSpace) {
        if self.tick_i % 70224 == 0 {
            let mut full_image = [[0u8; 256]; 256];
            let (bg_map, unique_tiles) = self.get_background_tile_map(memory);
            let mut tiles: HashMap<u16, [[u8; 8]; 8]> = HashMap::new();
            for tile in unique_tiles {
                tiles.insert(tile, self.get_tile(memory, tile));
            }

            for j in 0..32 {
                let tile_y0 = j * 8;
                for i in 0..32 {
                    let tile_x0 = i * 8;
                    let tile_idx = &bg_map[j][i];
                    // full_image[tile_y0..tile_y0 + 8][tile_x0..tile_x0 + 8] = tiles[tile_idx];
                    for y in 0..8 {
                        for x in 0..8 {
                            full_image[tile_y0 + y][tile_x0 + x] = tiles[tile_idx][y][x];
                        }
                    }
                }
            }

            self.canvas.clear();

            let view_y0 = self.scy(memory);
            let view_x0 = self.scx(memory);

            let margin_x = 255 - view_x0;
            let margin_y = 255 - view_y0;

            let fit_x = min(SCREEN_WIDTH, margin_x);
            let fit_y = min(SCREEN_HEIGHT, margin_y);

            // for j in 0..256 as i32 {
            //     for i in 0..256 as i32 {
            //         let color = full_image[j as usize][i as usize];
            //         self.canvas.set_draw_color(Color::RGB(color, color, color));
            //         self.canvas.draw_point(Point::new(i, j)).unwrap();
            //     }
            // }

            // self.image[:fit_y, :fit_x] = full_image[view_y0: view_y0 + fit_y, view_x0: view_x0 + fit_x]
            for j in 0..SCREEN_HEIGHT {
                let src_row;
                if j < fit_y {
                    src_row = full_image[view_y0 + j as usize];
                } else {
                    src_row = full_image[margin_y + j as usize];
                }
                for i in 0..SCREEN_WIDTH {
                    let color;
                    if i < fit_x {
                        color = src_row[view_x0 + i as usize];
                    } else {
                        color = src_row[margin_x + i as usize];
                    }
                    self.canvas.set_draw_color(Color::RGB(color, color, color));
                    self.canvas.draw_point(Point::new(i as i32, j as i32)).unwrap();
                }
            }

            self.canvas.present();


            // self.canvas.set_draw_color(Color::RGB(random(), random(), random()));
            // self.canvas.clear();
            // self.canvas.present();
        }
    }
}