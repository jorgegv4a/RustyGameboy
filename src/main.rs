mod registers;
mod cpu;

use cpu::CPU;

use std::io::Read;
use std::path::Path;
use std::fs::File;
use std::fs;

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

    let cpu = CPU::new();
    println!("CPU: {}", cpu)
}