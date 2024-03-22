#![allow(non_snake_case)]
mod registers;
mod cpu;
mod memory;
mod opcodes;

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
    let t0 = Instant::now();
    cpu.memory.load_rom(buf, memory::Cartridge::RomOnly);
    println!("elapsed: {}", t0.elapsed().as_micros());
    cpu.run();
    println!("CPU: {}", cpu);
    println!("rom data read @ 0x8123: {}", cpu.memory.read(0x8123));
    cpu.memory.write(0x8123, 41);
    println!("rom data read @ 0x8123: {}", cpu.memory.read(0x8123));
}