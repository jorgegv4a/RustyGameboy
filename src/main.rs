#![allow(non_snake_case)]
mod registers;
mod cpu;
mod memory;

use cpu::CPU;

use std::io::Read;
use std::path::Path;
use std::fs::File;
use std::fs;
use std::time::Instant;

fn main() {
    let path = Path::new("/home/mojonero/Downloads/Tetris (JUE) (V1.0).gb");
    let mut file = match File::open(&path) {
        Err(er) => panic!("Error found: '{}'", er),
        Ok(file) => file,
    };
    println!("Ayy");

    // let content = match fs::read(&path) {
    //     Err(er) => panic!("ayy limao: {}", er),
    //     Ok(_) => todo!(),
    // };
    let mut buf = Vec::new();
    let content = match file.read_to_end(&mut buf) {
        Err(er) => panic!("Error found: '{}'", er),
        Ok(file) => file,
    };
    println!("bytes: {}", buf[0]);

    let mut cpu = CPU::new();
    println!("rom data @ 1234: {}", cpu.memory.rom_bank[1234]);
    let t0 = Instant::now();
    cpu.memory.load_rom(buf, memory::Cartridge::RomOnly);
    println!("elapsed: {}", t0.elapsed().as_micros());
    println!("CPU: {}", cpu);
    println!("rom data @ 1234: {}", cpu.memory.rom_bank[1234]);
    println!("rom data read @ 1234: {}", cpu.memory.read(1234));
}