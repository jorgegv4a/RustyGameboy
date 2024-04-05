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

use std::path::Path;
// 
fn main() {
    // let path = Path::new("/home/mojonero/Downloads/Pokemon - Red Version (UE)/Pokemon Red.gb");
    // let path = Path::new("/home/mojonero/Downloads/Jeopardy! (USA).gb");
    // let path = Path::new("/home/mojonero/Downloads/mts-20240127-1204-74ae166/emulator-only/mbc1/bits_mode.gb");
    // let path = Path::new("/home/mojonero/Downloads/mts-20240127-1204-74ae166/acceptance/ppu/stat_irq_blocking.gb.gb");
    // let path = Path::new("/home/mojonero/Downloads/mts-20240127-1204-74ae166/acceptance/ppu/stat_lyc_onoff.gb");
    let path = Path::new("/home/mojonero/Downloads/Tetris (JUE) (V1.0).gb");
    // let path = Path::new("/home/mojonero/Downloads/dmg-acid2.gb");
    // let path = Path::new("/home/mojonero/PycharmProjects/gb_emulator/demo_roms/01-special.gb"); // PASSED
    // let path = Path::new("/home/mojonero/PycharmProjects/gb_emulator/demo_roms/02-interrupts.gb"); // failed EI - missing timer
    // let path = Path::new("/home/mojonero/PycharmProjects/gb_emulator/demo_roms/03-op sp,hl.gb"); // PASSED
    // let path = Path::new("/home/mojonero/PycharmProjects/gb_emulator/demo_roms/04-op r,imm.gb"); // PASSED
    // let path = Path::new("/home/mojonero/PycharmProjects/gb_emulator/demo_roms/05-op rp.gb"); // PASSED
    // let path = Path::new("/home/mojonero/PycharmProjects/gb_emulator/demo_roms/06-ld r,r.gb"); // PASSED
    // let path = Path::new("/home/mojonero/PycharmProjects/gb_emulator/demo_roms/07-jr,jp,call,ret,rst.gb"); // PASSED
    // let path = Path::new("/home/mojonero/PycharmProjects/gb_emulator/demo_roms/08-misc instrs.gb"); // PASSED
    // let path = Path::new("/home/mojonero/PycharmProjects/gb_emulator/demo_roms/09-op r,r.gb"); // PASSED
    // let path = Path::new("/home/mojonero/PycharmProjects/gb_emulator/demo_roms/10-bit ops.gb"); // PASSED
    // let path = Path::new("/home/mojonero/PycharmProjects/gb_emulator/demo_roms/11-op a,(hl).gb"); // PASSED

    let mut gb = gameboy::Gameboy::new();
    gb.load_game(&path);
    gb.power_on();

    // use std::time::Instant;
    // let t0 = Instant::now();
    // println!("elapsed: {}", t0.elapsed().as_micros());
}