use std::fs::File;
use std::io::Read;
use std::path::Path;

use crate::cpu::{CPU, DEBUG};
use crate::memory::{AddressSpace, Cartridge};
use crate::graphics::PPU;

pub struct Gameboy {
    cpu: CPU,
    memory: AddressSpace,
    // ppu: PPU,
}

impl Gameboy {
    pub fn new() -> Gameboy {
        Gameboy {
            cpu: CPU::new(),
            memory: AddressSpace::new(),
            // ppu: PPU::new(),
        }
    }

    pub fn load_game(&mut self, path: &Path) {
        let mut file = match File::open(path) {
            Err(er) => panic!("Error found: '{}'", er),
            Ok(file) => file,
        };
        let mut buf = Vec::new();
        let content = match file.read_to_end(&mut buf) {
            Err(er) => panic!("Error found: '{}'", er),
            Ok(file) => file,
        };
        self.memory.load_rom(buf, Cartridge::RomOnly);
    }

    pub fn power_on(&mut self) {
        self.cpu.boot(&mut self.memory);
        loop {
            if DEBUG {
                println!("{}", self.cpu);
            }
            let opcode_byte = self.cpu.fetch(&self.memory);
            let (opcode_dict, opcode) = self.cpu.decode(opcode_byte, &self.memory);
            let nticks = self.cpu.execute(opcode, opcode_dict, &mut self.memory);

            self.cpu.tick(nticks);
            self.memory.tick(nticks);
            if self.memory.read(0xFF02) == 0x81 {
                if !DEBUG {
                    print!("{}", std::char::from_u32(self.memory.read(0xFF01) as u32).unwrap_or('?'));
                }
                self.memory.write(0xFF02, 0);
            }

        }
    }
}