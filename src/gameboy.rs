use std::fs::File;
use std::io::Read;
use std::path::Path;

use crate::constants::{IF_ADDR, IE_ADDR, SB_ADDR, SC_ADDR};
use crate::cpu::{CPU, DEBUG};
use crate::memory::{AddressSpace, Cartridge};
use crate::graphics::PPU;
use crate::interrupt::Interrupt;

pub struct Gameboy {
    cpu: CPU,
    memory: AddressSpace,
    ppu: PPU,
}

impl Gameboy {
    pub fn new() -> Gameboy {
        let sdl_context = sdl2::init().unwrap();
        let video_subsystem = sdl_context.video().unwrap();
        Gameboy {
            cpu: CPU::new(),
            memory: AddressSpace::new(),
            ppu: PPU::new(video_subsystem),
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

    fn check_interrupts(&self) -> Option<Interrupt> {
        let interrupt_flags = self.memory.read(IF_ADDR);
        let interrupt_enables = self.memory.read(IE_ADDR);
        if !self.cpu.master_interrupt_enable {
            return None
        }

        for interrupt_i in 0..5 {
            if (interrupt_flags << interrupt_i) & 1 == 1 && (interrupt_enables << interrupt_i) & 1 == 1 {
                return Some(Interrupt::from(interrupt_i))
            };
        };
        None
    }

    fn serve_interrupt(&mut self, interrupt: Interrupt) {
        self.cpu.master_interrupt_enable = false;
        let mut interrupt_flags = self.memory.read(IF_ADDR);
        interrupt_flags = (interrupt_flags << interrupt as usize) & (0xFF ^ (1 << interrupt as usize));
        self.memory.write(IF_ADDR, interrupt_flags);
        
        let address = 0x40 + 8 * interrupt as u16;
        self.cpu.push_stack(self.cpu.registers.PC(), &mut self.memory);
        self.cpu.registers.write_PC(address);

    }

    pub fn power_on(&mut self) {
        self.cpu.boot(&mut self.memory);
        loop {
            if DEBUG {
                println!("{}", self.cpu);
            }
            if let Some(interrupt) = self.check_interrupts() {
                self.serve_interrupt(interrupt);
            }

            let opcode_byte = self.cpu.fetch(&self.memory);
            let (opcode_dict, opcode) = self.cpu.decode(opcode_byte, &self.memory);
            let nticks = self.cpu.execute(opcode, opcode_dict, &mut self.memory);

            self.cpu.tick(nticks);
            self.memory.tick(nticks);
            self.ppu.tick(nticks, &mut self.memory);
            if self.memory.read(SC_ADDR) == 0x81 {
                if !DEBUG {
                    print!("{}", std::char::from_u32(self.memory.read(SB_ADDR) as u32).unwrap_or('?'));
                }
                self.memory.write(SC_ADDR, 0);
            }

        }
    }
}