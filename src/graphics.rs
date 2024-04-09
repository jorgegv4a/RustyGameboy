extern crate sdl2;

use sdl2::pixels::Color;
use sdl2::VideoSubsystem;
use std::collections::HashSet;
use std::time::Duration;
use std::cmp::{min, max};
use sdl2::rect::Point;
use std::time::Instant;
use std::convert::Into;

use crate::constants::*;
use crate::interrupt::Interrupt;
use crate::memory::AddressSpace;
use crate::sprites::*;


fn get_bg_win_display(memory: &AddressSpace) -> bool {
    return (memory.read(LCDC_ADDR) >> LCDC_BG_WIN_DISPLAY_BIT) & 1 == 1;
}

fn get_win_tile_map(memory: &AddressSpace) -> bool {
    return (memory.read(LCDC_ADDR) >> LCDC_WINDOW_TILE_MAP_BIT) & 1 == 1;
}

fn get_bg_win_tile_data_zone(memory: &AddressSpace) -> bool {
    return (memory.read(LCDC_ADDR) >> LCDC_BG_WINDOW_TILE_DATA_AREA_BIT) & 1 == 1;
}

fn get_bg_tile_map(memory: &AddressSpace) -> bool {
    return (memory.read(LCDC_ADDR) >> LCDC_BG_TILE_MAP_BIT) & 1 == 1;
}

fn get_obj_size(memory: &AddressSpace) -> bool {
    return (memory.read(LCDC_ADDR) >> LCDC_OBJ_SIZE_BIT) & 1 == 1;
}

fn get_ppu_enabled(memory: &AddressSpace) -> bool {
    return (memory.read(LCDC_ADDR) >> LCDC_PPU_ENABLE_BIT) & 1 == 1;
}

fn get_obj_enabled(memory: &AddressSpace) -> bool {
    return (memory.read(LCDC_ADDR) >> LCDC_OBJ_ENABLE_BIT) & 1 == 1;
}

fn scx(memory: &AddressSpace) -> usize {
    return memory.read(SCX_ADDR) as usize;
}

fn scy(memory: &AddressSpace) -> usize {
    return memory.read(SCY_ADDR) as usize;
}

fn wx(memory: &AddressSpace) -> usize {
    return memory.read(WX_ADDR) as usize;
}

fn wy(memory: &AddressSpace) -> usize {
    return memory.read(WY_ADDR) as usize;
}

fn window_enabled(memory: &AddressSpace) -> bool {
    let lcdc = memory.read(LCDC_ADDR);
    let win_en = (lcdc >> LCDC_WINDOW_ENABLE_BIT) & 1 == 1;
    let win_gb_en = get_bg_win_display(memory);
    return win_en && win_gb_en;
}


#[derive(Debug, PartialEq)]
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
    render_window_on_cur_frame: bool,
    wly: usize,
    stat_flag: bool,
    past_cycle_disabled: bool,
    frame_start_t: Instant,
    past_tick_lyc: Option<u8>,
}

impl PPU {
    pub fn new(sdl_subsystem: VideoSubsystem, window_scale: f32) -> PPU {
        let window = sdl_subsystem
            .window("rust-sdl2 demo: Video", (SCREEN_WIDTH as f32 * window_scale) as u32, (SCREEN_HEIGHT as f32 * window_scale) as u32)
            .position_centered()
            .opengl()
            .build()
            .map_err(|e| e.to_string()).unwrap();
        let mut canvas = window.into_canvas().build().map_err(|e| e.to_string()).unwrap();
        canvas.set_scale(window_scale, window_scale);
        PPU {
            dot: 0,
            ly: 0,
            line_objects: Vec::new(),
            mode: PPUMode::OAMScan,
            tick_i: 0,
            canvas,
            render_window_on_cur_frame: false,
            wly: 0,
            past_cycle_disabled: false,
            frame_start_t: Instant::now(),
            stat_flag: false,
            past_tick_lyc: None,
        }
    }

    fn check_stat_irq(&self, memory: &AddressSpace) -> bool {
        let value = memory.read(STAT_ADDR);
        let interrupt_on_equal_lyc = (value >> 6) & 1 == 1;

        if (value >> 2) & 1 == 1 && interrupt_on_equal_lyc {
                return true;
        }
        if (value >> 3) & 1 == 1 && self.mode == PPUMode::VBlank {
            return true;
        } else if (value >> 4) & 1 == 1 && self.mode == PPUMode::HBlank {
            return true;
        } else if (value >> 5) & 1 == 1 && self.mode == PPUMode::OAMScan {
            return true;
        } else {
            return false;
        }
    } 
    

    fn single_tick(&mut self, memory: &mut AddressSpace) {
        if !get_ppu_enabled(memory) {
            // self.mode = PPUMode::HBlank;
            self.stat_flag = false;
            // memory.ppu_write_LY(0);
            // memory.unlock_oam();
            // memory.unlock_vram();
            self.past_cycle_disabled = true;
            return
        }
        /*
        Conditions for STAT IRQ to enable:
            LY == LYC
            Any of the modes matches enabled mode
        
        Actions that could modify STAT IRQ:
            - LY updates
            - LYC writes
            - Mode changes
            - Enabled mode changes

        Condition for STAT IRQ Interrupt:
            - STAT IRQ transition from low to high
         */
        if self.past_cycle_disabled {
            self.ly = 0;
            self.dot = 0;
            self.mode = PPUMode::OAMScan;
            memory.lock_oam();
            memory.ppu_write_LY_update_STAT(self.ly);
            // memory.ppu_write_LY(self.ly);
            self.past_cycle_disabled = false;
        }
        let lyc = memory.read(LYC_ADDR);
        if let Some(x) = self.past_tick_lyc {
            if x != lyc {
                memory.ppu_write_LY_update_STAT(self.ly)
            }
        }
        match self.mode {
            PPUMode::OAMScan => {
                if !self.render_window_on_cur_frame && self.ly == wy(memory) as u8 && window_enabled(memory) {
                    self.render_window_on_cur_frame = true;
                    self.wly = 0;
                }
                if self.dot == 0 {
                    memory.ppu_write_LY_update_STAT(self.ly);
                    if self.ly == 0 {
                        let frame_time_seconds = self.frame_start_t.elapsed().as_secs_f64();
                        let fps = 1.0 / frame_time_seconds;
                        if frame_time_seconds < 1.0 / 60.0 {
                            let diff = 1.0 / 60.0 - frame_time_seconds;
                            std::thread::sleep(Duration::from_secs_f64(diff));
                        }
                        println!("Frame time: {:.3} ms, FPS: {fps:.1}", frame_time_seconds * 1000.0);
                        self.frame_start_t = Instant::now();
                    }
                }
                if self.dot == 80 {
                    self.mode = PPUMode::Drawing;

                    let sprite_height: u8 = if get_obj_size(memory) == false {8} else {16};
                    let sprites_disabled = !get_obj_enabled(memory);

                    for sprite_i in 0..40 {
                        let sprite = SpriteData::new((sprite_i * 4) as u8, memory);
                        let short_sprite_hidden = sprite.y < 8 && sprite_height == 8;
                        let sprite_hidden_0 = sprite.y == 0;
                        let sprite_hidden_160 = sprite.y == 160;
                        let sprites_full = self.line_objects.len() == 10;
                        let stop_scanning = sprites_disabled || short_sprite_hidden || sprite_hidden_0 || sprite_hidden_160 || sprites_full;
                        if !stop_scanning{
                            if self.ly + 16 >= sprite.y && self.ly + 16 < sprite.y + sprite_height {
                                self.line_objects.push(sprite);
                            }
                        }
                    }
                } else {
                    self.dot += 1;
                }
            },
            PPUMode::Drawing => {
                if self.dot == 80 {
                    // memory.lock_oam();
                    // memory.lock_vram();
                }
                if self.dot == 80 + 172 { 
                    self.mode = PPUMode::HBlank;
                    memory.unlock_oam();
                    memory.unlock_vram();
                    self.show(memory);
                }
                self.dot += 1;
            },
            PPUMode::HBlank => {
                if self.dot == NUM_DOTS_PER_LINE { 
                    self.dot = 0; 
                    self.ly += 1; 
                    if self.ly == 144 { 
                        self.mode = PPUMode::VBlank;
                        memory.unlock_oam();
                        memory.request_interrupt(Interrupt::VBlank);
                    } else { 
                        if self.render_window_on_cur_frame && wx(memory) <= 166 {
                            self.wly += 1;
                        }
                        self.mode = PPUMode::OAMScan;
                        self.line_objects.clear();
                        // memory.lock_oam();
                    };
                    memory.ppu_write_LY_update_STAT(self.ly);
                } else {
                    self.dot += 1;
                }
            },
            PPUMode::VBlank => {
                if self.dot == NUM_DOTS_PER_LINE { 
                    self.dot = 0; 
                    self.ly += 1; 
                    memory.ppu_write_LY_update_STAT(self.ly);
                } else {
                    self.dot += 1;
                }
                if self.ly == NUM_SCAN_LINES - 1 { 
                    self.ly = 0; 
                    self.mode = PPUMode::OAMScan;
                    self.line_objects.clear();
                    self.render_window_on_cur_frame = false;
                    self.wly = 0;
                    // memory.lock_oam();
                    memory.ppu_write_LY_update_STAT(self.ly);
                };
            },
        };
    self.update_stat(memory);
    if !self.stat_flag && self.check_stat_irq(memory) {
        memory.request_interrupt(Interrupt::LCD);
    } else {
        // println!("\r");
    }
    self.stat_flag = self.check_stat_irq(memory);
    self.past_tick_lyc = Some(lyc);
    self.tick_i += 1
    }

    fn update_stat(&mut self, memory: &mut AddressSpace) {
        memory.ppu_write_LY_update_STAT(self.ly);
        let mut value = memory.read(STAT_ADDR);

        if (value >> 3) & 1 == 1 && self.mode == PPUMode::VBlank {
            value |= 1 << 3;
        } else {
            value &= 0xFF ^ (1 << 3);
        }
        if (value >> 4) & 1 == 1 && self.mode == PPUMode::HBlank {
            value |= 1 << 4;
        } else {
            value &= 0xFF ^ (1 << 4);
        }
        if (value >> 5) & 1 == 1 && self.mode == PPUMode::OAMScan {
            value |= 1 << 5;
        } else {
            value &= 0xFF ^ (1 << 5);
        }

        memory.ppu_write_stat(value);

    }

    pub fn tick(&mut self, nticks: u8, memory: &mut AddressSpace) {
        for _ in 0..nticks {
            self.single_tick(memory);
        }
    }

    fn _get_background_tile_map(&self, memory: &AddressSpace) -> ([[u16; 32]; 32], HashSet<u16>) {
        let mut tile_map = [[0u16; 32]; 32];
        let tile_map_base_address = if get_bg_tile_map(memory) { 0x9C00 } else { 0x9800 };
        let mut unique_tiles: HashSet<u16> = HashSet::new();
        for tile_map_idx in 0..1024 {
            let tile_x = tile_map_idx % 32;
            let tile_y = tile_map_idx / 32;
            let tile_offset = memory.read(tile_map_base_address + tile_map_idx) as u16;
            let tile_address;
            if get_bg_win_tile_data_zone(memory) {
                tile_address = 0x8000 + tile_offset * 16;
            } else {
                tile_address = (0x8800 + ((tile_offset as i32) - 128) * 16) as u16
            }
            tile_map[tile_y as usize][tile_x as usize] = tile_address;
            unique_tiles.insert(tile_address);
        }
        (tile_map, unique_tiles)
    }

    fn get_tile(&self, memory: &AddressSpace, tile_start_i: u16) -> [[u8; 8]; 8] {
        let mut tile: [[u8; 8]; 8] = [[0u8; 8]; 8];
        for j in 0..8 {
            let tile_low_byte = memory.read(tile_start_i + 2 * j);
            let tile_high_byte = memory.read((tile_start_i + 2 * j) + 1);
            for i in 0..8 {
                let low_byte = (tile_low_byte >> i) & 1;
                let high_byte = ((tile_high_byte >> i) & 1) << 1;
                let color_id = min(3, max(0, high_byte | low_byte));
                // let color = (((color_id as f32) / 3f32) * 255f32) as u8;
                // tile[j as usize][(7 - i) as usize] = color;
                tile[j as usize][(7 - i) as usize] = color_id;
            }
        };
        tile
    }

    fn get_double_tile(&self, memory: &AddressSpace, tile_start_i: u16) -> [[u8; 8]; 16] {
        let mut tile: [[u8; 8]; 16] = [[0u8; 8]; 16];
        for j in 0..16 {
            let tile_low_byte = memory.read(tile_start_i + 2 * j);
            let tile_high_byte = memory.read((tile_start_i + 2 * j) + 1);
            for i in 0..8 {
                let low_byte = (tile_low_byte >> i) & 1;
                let high_byte = ((tile_high_byte >> i) & 1) << 1;
                let color_id = min(3, max(0, high_byte | low_byte));
                // let color = (((color_id as f32) / 3f32) * 255f32) as u8;
                // tile[j as usize][(7 - i) as usize] = color;
                tile[j as usize][(7 - i) as usize] = color_id;
            }
        };
        tile
    }

    fn show(&mut self, memory: &AddressSpace) {
        let line_j = self.ly as usize;

        let bg_tile_map_base_address: u16 = if get_bg_tile_map(memory) { 0x9C00 } else { 0x9800 };
        let win_tile_map_base_address: u16 = if get_win_tile_map(memory) { 0x9C00 } else { 0x9800 };
        let direct_data_zone = get_bg_win_tile_data_zone(memory);

        self.line_objects.sort_by(|a, b| a.cmp(b));

        let sprite_height: usize = if get_obj_size(memory) == false {8} else {16};

        let BPG = memory.read(BGP_ADDR);
        let OBP0 = memory.read(OBP0_ADDR);
        let OBP1 = memory.read(OBP1_ADDR);

        let bpg_palette = make_palette(BPG);
        let obp0_palette = make_palette(OBP0);
        let obp1_palette = make_palette(OBP1);

        // let window_x_condition = self.wx(memory) <= 166;
        let win_x = wx(memory);
        let win_enabled = (memory.read(LCDC_ADDR) >> LCDC_WINDOW_ENABLE_BIT) & 1 == 1;

        let view_y0 = scy(memory);
        let view_x0 = scx(memory);


        let margin_x = 256 - view_x0;
        let margin_y = 256 - view_y0;

        let fit_x = min(SCREEN_WIDTH, margin_x);
        let fit_y = min(SCREEN_HEIGHT, margin_y);

        for i in 0..SCREEN_WIDTH {
            let bg_color: u8;
            if self.render_window_on_cur_frame && i + 7 >= win_x && win_enabled {
                let src_row = self.wly;
                let tile_y = src_row / 8;
                let tile_offset_y = src_row % 8;

                let col_row = i + 7 - wx(memory);
                let tile_x = col_row / 8;
                let tile_offset_x = col_row % 8;

                let tile_map_idx = tile_y * 32 + tile_x;
                let tile_offset = memory.read(win_tile_map_base_address + tile_map_idx as u16) as u16;
                let tile_idx: u16 = if direct_data_zone {
                    0x8000 + tile_offset * 16
                } else {
                    if tile_offset > 127 {
                        0x8000 + tile_offset * 16
                    } else {
                        0x9000 + tile_offset * 16
                    }  
                };
                let tile = self.get_tile(memory, tile_idx);
                bg_color = tile[tile_offset_y][tile_offset_x];
            } else {
                let src_row;
                if line_j < fit_y {
                    src_row = view_y0 + line_j;
                } else {
                    src_row = line_j - fit_y;
                }
                let tile_y = src_row / 8;
                let tile_offset_y = src_row % 8;

                let src_col;
                if i < fit_x {
                    src_col = view_x0 + i;
                } else {
                    src_col = i - fit_x;
                }
                let tile_x = src_col / 8;
                let tile_offset_x = src_col % 8;

                let tile_map_idx = tile_y * 32 + tile_x;
                let tile_offset = memory.read(bg_tile_map_base_address + tile_map_idx as u16) as u16;
                let tile_idx: u16 = if direct_data_zone {
                    0x8000 + tile_offset * 16
                } else {
                    if tile_offset > 127 {
                        0x8000 + tile_offset * 16
                    } else {
                        0x9000 + tile_offset * 16
                    }
                };
                let tile = self.get_tile(memory, tile_idx);
                bg_color = tile[tile_offset_y][tile_offset_x];
            }

            let mut sprite_pixel: Option<(u8, ColorPalette, bool)> = None;
            for sprite in self.line_objects.iter() {
                if i + 8 >= sprite.x as usize && i < sprite.x as usize {
                    let mut sprite_idx = sprite.raw_tile_index;
                    if sprite_height == 16 {
                        sprite_idx &= 0xFE;
                    }
                    let sprite_tile_idx = 0x8000 + (sprite_idx as u16) * 16;

                    let within_sprite_y = line_j + 16 - sprite.y as usize;
                    
                    let sprite_row;
                    if sprite_height == 8 {
                        let sprite_tile = self.get_tile(memory, sprite_tile_idx);
                        if sprite.attrs.y_flip {
                            sprite_row = sprite_tile[sprite_height - 1 - within_sprite_y]
                        } else {
                            sprite_row = sprite_tile[within_sprite_y]
                        }
                    } else {
                        let sprite_tile = self.get_double_tile(memory, sprite_tile_idx);
                        if sprite.attrs.y_flip {
                            sprite_row = sprite_tile[sprite_height - 1 - within_sprite_y]
                        } else {
                            sprite_row = sprite_tile[within_sprite_y]
                        }
                    }

                    let within_sprite_x = i + 8 - sprite.x as usize;
                    let sprite_color;
                    if sprite.attrs.x_flip {
                        sprite_color = sprite_row[7 - within_sprite_x]
                    } else {
                        sprite_color = sprite_row[within_sprite_x]
                    }
                    if sprite_color != 0 {
                        sprite_pixel = Some((sprite_color, sprite.attrs.palette, sprite.attrs.priority));
                        break
                    }
                }
            }

            let color: u8;
            let color_palette: ColorPalette;
            if sprite_pixel.is_some() {
                let (sprite_color, sprite_palette, sprite_prio) = sprite_pixel.unwrap();
                if get_bg_win_display(memory) {
                    if sprite_color == 0 {
                        color = bg_color;
                        color_palette = ColorPalette::BPG;
                    } else if sprite_prio && bg_color != 0 {
                        color = bg_color;
                        color_palette = ColorPalette::BPG;
                    } else {
                        color = sprite_color;
                        color_palette = sprite_palette;
                    }
                } else {
                    color = sprite_color;
                    color_palette = sprite_palette;
                }
            } else {
                if get_bg_win_display(memory) {
                    color = bg_color;
                    color_palette = ColorPalette::BPG;
                } else {
                    color = 0;
                    color_palette = ColorPalette::BPG;
                }
            }
            // color = bg_color;
            let color_value = match (color_palette, color) {
                // (ColorPalette::BPG, 4) => Color::RGB(255, 255, 255),
                (ColorPalette::BPG, _) => bpg_palette[color as usize].into(),
                (ColorPalette::OBP1, 0) => Color::RGB(0, 255, 0),
                (ColorPalette::OBP0, 0) => Color::RGB(0, 255, 0),
                (ColorPalette::OBP0, _) => obp0_palette[color as usize].into(),
                (ColorPalette::OBP1, _) => obp1_palette[color as usize].into(),
            };
            
            self.canvas.set_draw_color(color_value);
            self.canvas.draw_point(Point::new(i as i32, line_j as i32)).unwrap();
        }

        self.canvas.present();
    }
}

fn make_palette(packed_palette: u8) -> [ColorId; 4] {
    let mut palette: [ColorId; 4] = [ColorId::Zero; 4];
    for i in 0..4 {
        let color = (packed_palette >> i * 2) & 0x3;
        palette[i] = ColorId::from(color);
    }
    palette
}