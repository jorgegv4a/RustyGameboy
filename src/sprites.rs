extern crate sdl2;

use sdl2::pixels::Color;

use std::cmp::Ordering;

use crate::memory::AddressSpace;

use std::convert::Into;

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum ColorId {
    Zero = 0,
    One = 1,
    Two = 2,
    Three = 3,
    Debug = 4,
    Blank = 5,
}

impl Into<Color> for ColorId {
    fn into(self) -> Color {
        match self {
            ColorId::Zero => Color::RGB(255, 255, 255),
            ColorId::One => Color::RGB(170, 170, 170),
            ColorId::Two => Color::RGB(85, 85, 85),
            ColorId::Three => Color::RGB(0, 0, 0),
            ColorId::Debug => Color::RGB(255, 0, 0),
            ColorId::Blank => Color::RGB(255, 255, 255),
        }
    }
}

impl From<u8> for ColorId {
    fn from(value: u8) -> Self {
        match value {
            0 => ColorId::Zero,
            1 => ColorId::One,
            2 => ColorId::Two,
            3 => ColorId::Three,
            _ => panic!("Invalid value for palette: {value} range (0-3)"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum ColorPalette {
    BPG,
    OBP0,
    OBP1,
}

#[derive(Debug)]
pub(crate) struct SpriteData {
    pub(crate) x: u8,
    pub(crate) y: u8,
    pub(crate) raw_tile_index: u8,
    pub(crate) attrs: SpriteAttributes,
    pub(crate) oam_index: u8,
}

impl SpriteData {
    pub(crate) fn new(sprite_index: u8, memory: &mut AddressSpace) -> SpriteData {
        let sprite_bytes = memory.read_sprite(sprite_index);
        SpriteData {
            y: sprite_bytes[0],
            x: sprite_bytes[1],
            raw_tile_index: sprite_bytes[2],
            attrs: SpriteAttributes::new(sprite_bytes[3]),
            oam_index: sprite_index,
        }
    }

    pub(crate) fn wins_prio(&self, other: &Self) -> bool {
        if self.x == other.x {
            return self.oam_index < other.oam_index;
        } else {
            return self.x < other.x;
        }
    }
    pub(crate) fn cmp (&self, other: &Self) -> Ordering {
        match self.wins_prio(other) {
            true => Ordering::Less,
            false => Ordering::Greater,
        }
    }
}

#[derive(Debug)]
pub(crate) struct SpriteAttributes {
    pub(crate) priority: bool,
    pub(crate) x_flip: bool,
    pub(crate) y_flip: bool,
    pub(crate) palette: ColorPalette,
}

impl SpriteAttributes {
    pub(crate) fn new(attrs: u8) -> SpriteAttributes {
        SpriteAttributes {
            priority: (attrs >> 7) & 1 == 1,
            x_flip: (attrs >> 5) & 1 == 1,
            y_flip: (attrs >> 6) & 1 == 1,
            palette: if (attrs >> 4) & 1 == 1 { ColorPalette::OBP1 } else { ColorPalette::OBP0 },
        }
    }
}
