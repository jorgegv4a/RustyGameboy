extern crate sdl2;

use sdl2::pixels::Color;
use sdl2::VideoSubsystem;
use std::collections::HashSet;
use std::time::Duration;
use std::collections::HashMap;
use std::cmp::min;
use sdl2::rect::Point;
use std::time::Instant;

use crate::constants::*;
use crate::interrupt::Interrupt;
use crate::memory::AddressSpace;

#[derive(Debug)]
struct SpriteData {
    x: u8,
    y: u8,
    raw_tile_index: u8,
    attrs: u8
}

impl SpriteData {
    fn new(sprite_index: u8, memory: &mut AddressSpace) -> SpriteData {
        let sprite_bytes = memory.read_sprite(sprite_index);
        SpriteData {
            y: sprite_bytes[0],
            x: sprite_bytes[1],
            raw_tile_index: sprite_bytes[2],
            attrs: sprite_bytes[3],
        }
    }
}

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
    line_objects: Vec<SpriteData>,
    mode: PPUMode,
    tick_i: u64,
    canvas: sdl2::render::Canvas<sdl2::video::Window>,
}

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
            line_objects: Vec::new(),
            mode: PPUMode::OAMScan,
            tick_i: 0,
            canvas,
        }
    }

    fn get_bg_win_display(&self, memory: &AddressSpace) -> bool {
        return (memory.read(LCDC_ADDR) >> LCDC_BG_WIN_DISPLAY_BIT) & 1 == 1;
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

    fn get_stat_mode(&self, memory: &AddressSpace) -> u8 {
        return (memory.read(STAT_ADDR) >> 2) & 7;
    }

    fn single_tick(&mut self, memory: &mut AddressSpace) {
        if !self.get_ppu_enabled(memory) {
            return
        }
        // println!("PPU, dot: {}, ly: {}, mode: {:?}", self.dot, self.ly, self.mode);
        let stat_modes = self.get_stat_mode(memory);
        match self.mode {
            PPUMode::OAMScan => {
                if self.dot == 80 {
                    self.mode = PPUMode::Drawing;
                    memory.lock_oam();
                    memory.request_interrupt(Interrupt::LCD);
                } else {
                    let sprite = SpriteData::new(self.dot as u8, memory);
                    let sprite_height: u8 = if self.get_obj_size(memory) == false {8} else {16};
                    let stop_scanning = !self.get_obj_enabled(memory) || (sprite.y < 8 && sprite_height == 8) || sprite.y == 0 || sprite.y >= 160 || self.line_objects.len() == 10;
                    if !stop_scanning && (self.ly >= sprite.y && self.ly < sprite.y + sprite_height){
                        self.line_objects.push(sprite);
                    }
                    self.dot += 1;
                }
            },
            PPUMode::Drawing => {
                if self.dot == 80 + 172 { 
                    self.mode = PPUMode::HBlank;
                    memory.unlock_oam();
                    if stat_modes & 1 == 1 {
                        memory.request_interrupt(Interrupt::LCD);
                    }
                    self.show(memory);
                }
                self.dot += 1;
            },
            PPUMode::HBlank => {
                if self.dot == 456 { 
                    self.dot = 0; 
                    self.ly += 1; 
                    if self.ly >= 144 { 
                        self.mode = PPUMode::VBlank;
                        memory.unlock_oam();
                        memory.request_interrupt(Interrupt::VBlank);
                        
                        if (stat_modes >> 1) & 1 == 1 {
                            memory.request_interrupt(Interrupt::LCD);
                        }
                    } else { 
                        self.mode = PPUMode::OAMScan;
                        self.line_objects.clear();
                        memory.lock_oam();
                        if (stat_modes >> 2) & 1 == 1 {
                            memory.request_interrupt(Interrupt::LCD);
                        }
                    };
                } else {
                    self.dot += 1;
                }
            },
            PPUMode::VBlank => {
                if self.dot == 456 { 
                    self.dot = 0; 
                    self.ly += 1; 
                } else {
                    self.dot += 1;
                }
                if self.ly == 154 { 
                    self.ly = 0; 
                    self.mode = PPUMode::OAMScan;
                    self.line_objects.clear();
                    memory.lock_oam();
                    if (stat_modes >> 2) & 1 == 1 {
                        memory.request_interrupt(Interrupt::LCD);
                    }
                };
            },
        };

    self.set_ly(memory);

    // self.show(memory);
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
                let low_byte = (tile_low_byte >> i) & 1;
                let high_byte = ((tile_high_byte >> i) & 1) << 1;
                let color_id = high_byte | low_byte;
                let color = (((color_id as f32) / 3f32) * 255f32) as u8;
                tile[j as usize][(7 - i) as usize] = 255 - color;
            }
        };
        tile
    }

    fn show_bg(&mut self, memory: &AddressSpace) {
        if self.tick_i % 70224 == 0 {
            let t0 = Instant::now();
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
            println!("Frame time: {}", t0.elapsed().as_millis());
        }
    }

    fn show(&mut self, memory: &AddressSpace) {
        let t0 = Instant::now();
        let mut t1 = Instant::now();
        
        let line_j = self.ly as usize;

        let view_y0 = self.scy(memory);
        let view_x0 = self.scx(memory);

        let margin_x = 255 - view_x0;
        let margin_y = 255 - view_y0;

        let fit_x = min(SCREEN_WIDTH, margin_x);
        let fit_y = min(SCREEN_HEIGHT, margin_y);

        let tile_map_base_address: u16 = if self.get_bg_tile_map(memory) { 0x9C00 } else { 0x9800 };
        let direct_data_zone = self.get_bg_win_tile_data_zone(memory);

        for i in 0..SCREEN_WIDTH {
            let src_row;
            if line_j < fit_y {
                src_row = view_y0 + line_j;
            } else {
                src_row = margin_y + line_j;
            }
            let tile_y = src_row / 8;
            let tile_offset_y = src_row % 8;

            let src_col;
            if i < fit_x {
                src_col = view_x0 + i;
            } else {
                src_col = margin_x + i;
            }
            let tile_x = src_col / 8;
            let tile_offset_x = src_col % 8;

            let tile_map_idx = tile_y * 32 + tile_x;
            let tile_offset = memory.read(tile_map_base_address + tile_map_idx as u16) as u16;
            let tile_idx: u16 = if direct_data_zone {
                0x8000 + tile_offset * 16
            } else {
                (0x9000 + ((tile_offset as i32) - 128) * 16) as u16
            };
            let tile = self.get_tile(memory, tile_idx);
            let color = tile[tile_offset_y][tile_offset_x];
            self.canvas.set_draw_color(Color::RGB(color, color, color));
            self.canvas.draw_point(Point::new(i as i32, line_j as i32)).unwrap();
        }

        println!("Draw line: {}", t1.elapsed().as_micros());
        t1 = Instant::now();
        self.canvas.present();
        println!("Present line: {}", t1.elapsed().as_micros());
        println!("Frame time: {}\n", t0.elapsed().as_micros());
    }
}