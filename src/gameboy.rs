use std::fs::File;
use std::io::Read;
use std::path::Path;
use std::time::{Duration, Instant};
use sdl2::audio::{AudioCallback, AudioSpecDesired};

use crate::constants::{IF_ADDR, IE_ADDR, SB_ADDR, SC_ADDR};
use crate::cpu::{CPU, DEBUG};
use crate::memory::AddressSpace;
use crate::graphics::PPU;
use crate::interrupt::Interrupt;
use crate::joypad::Joypad;
use crate::sound::APU;


pub struct Gameboy {
    cpu: CPU,
    memory: AddressSpace,
    ppu: PPU,
    joypad: Joypad,
    apu: APU,
}

impl Gameboy {
    pub fn new() -> Gameboy {
        let sdl_context = sdl2::init().unwrap();
        let video_subsystem = sdl_context.video().unwrap();
        let mut event_pump = sdl_context.event_pump().unwrap();

        let audio_subsystem = sdl_context.audio().unwrap();

        Gameboy {
            cpu: CPU::new(),
            memory: AddressSpace::new(),
            ppu: PPU::new(video_subsystem, 3.0),
            joypad: Joypad::new(event_pump),
            apu: APU::new(audio_subsystem),
        }
    }

    pub fn load_game(&mut self, path: &Path) {
        let mut file = match File::open(path) {
            Err(er) => panic!("Error found: '{}'", er),
            Ok(file) => file,
        };
        let mut buf = Vec::new();
        let _content = match file.read_to_end(&mut buf) {
            Err(er) => panic!("Error found: '{}'", er),
            Ok(file) => file,
        };
        match self.memory.load_rom(buf) {
            Ok(x) => x,
            Err(s) => panic!("Failed to load game: {s}"),
        };
    }

    fn check_interrupts(&self) -> Option<Interrupt> {
        let interrupt_flags = self.memory.read(IF_ADDR);
        let interrupt_enables = self.memory.read(IE_ADDR);
        if interrupt_flags == 0 || interrupt_enables == 0 {
            return None
        }

        for interrupt_i in 0..5 {
            if (interrupt_flags >> interrupt_i) & 1 == 1 && (interrupt_enables >> interrupt_i) & 1 == 1 {
                return Some(Interrupt::from(interrupt_i))
            };
        };
        None
    }

    fn serve_interrupt(&mut self, interrupt: Interrupt) {
        self.cpu.master_interrupt_enable = false;
        let mut interrupt_flags = self.memory.read(IF_ADDR);
        interrupt_flags = interrupt_flags & (0xFF ^ (1 << interrupt as usize));
        self.memory.write(IF_ADDR, interrupt_flags);
        
        let address = 0x40 + 8 * interrupt as u16;
        self.cpu.push_stack(self.cpu.registers.PC(), &mut self.memory);
        self.cpu.registers.write_PC(address);

    }

    pub fn power_on(&mut self) {
        self.cpu.boot(&mut self.memory);
        loop {
            let start_t = self.cpu.clock;
            // let t0 = Instant::now();
            if DEBUG {
                println!("{}", self.cpu);
            }
            if self.cpu.enable_interrupts_next_instr {
                self.cpu.master_interrupt_enable = true;
                self.cpu.enable_interrupts_next_instr = false;
            }
            if let Some(interrupt) = self.check_interrupts() {
                if self.cpu.master_interrupt_enable {
                    if self.cpu.is_halted() {
                        self.cpu.quit_halt();
                    }
                    self.serve_interrupt(interrupt);
                } else {
                    if self.cpu.is_halted() {
                        self.cpu.quit_halt();
                    }
                }
            }

            let opcode_byte = self.cpu.fetch(&self.memory);
            let (opcode_dict, opcode) = self.cpu.decode(opcode_byte, &self.memory);
            let remaining_ticks = self.cpu.execute(opcode, opcode_dict, &mut self.memory);
            let nticks = (self.cpu.clock - start_t) as u8  + remaining_ticks;
            
            self.cpu.tick(remaining_ticks);
            self.ppu.tick(nticks, &mut self.memory);
            let quit = self.joypad.tick(nticks, &mut self.memory);
            self.memory.tick(nticks);
            self.apu.tick(nticks, &mut self.memory);
            if self.memory.read(SC_ADDR) == 0x81 {
                if !DEBUG {
                    print!("{}", std::char::from_u32(self.memory.read(SB_ADDR) as u32).unwrap_or('?'));
                }
                self.memory.write(SC_ADDR, 0);
            }
            if quit {
                return;
            }
            // for event in self.events.poll_iter() {
            //     match event {
            //         Event::Quit { .. }
            //         | Event::KeyDown {
            //             keycode: Some(Keycode::Escape),
            //             ..
            //         } => return,
            //         _ => {}
            //     }
            // }
        }
    }
}