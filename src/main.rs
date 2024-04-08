#![allow(non_snake_case)]
mod registers;
mod cpu;
mod memory;
mod opcodes;
mod constants;
mod graphics;
mod gameboy;
mod interrupt;
mod joypad;
mod sprites;
mod mappers;

use std::path::Path;
// 
fn main() {
    let path = Path::new("/Game/Path/*.gb");
    let mut gb = gameboy::Gameboy::new();
    gb.load_game(&path);
    gb.power_on();
}