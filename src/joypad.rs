use device_query::{DeviceQuery, DeviceState, MouseState, Keycode};
use crate::memory::{self, AddressSpace};
use crate::constants::*;
use crate::interrupt::Interrupt;

pub struct Joypad {
    state: u8,
    device_state: DeviceState,
    ticks: u64,
}

enum GBKeys {
    Right = 0,
    Left = 1,
    Up = 2,
    Down = 3,
    A = 4,
    B = 5,
    Select = 6,
    Start = 7,
}

impl Joypad {
    pub fn new() -> Joypad {
        let device_state = DeviceState::new();
        Joypad {
            state: 0xFF,
            device_state,
            ticks: 0,
        }
    }

    fn get_selection(&self, memory: &AddressSpace) -> u8 {
        return (memory.read(JOYP_ADDR) >> 4) & 0x3;
    }

    fn update_memory(&self, memory: &mut AddressSpace) {
        let selection = (self.get_selection(memory) >> 4) & 0x3;
        memory.joypad_write(self.state); 
    }

    fn update_state(&mut self, memory: &mut AddressSpace) {
        let keys: Vec<Keycode> = self.device_state.get_keys();
        let prev_state = self.state;
        if keys.contains(&Keycode::Right) || keys.contains(&Keycode::D) {
            self.state &= 1 ^ 0xFF;
        } else {
            self.state |= 1;
        }

        if keys.contains(&Keycode::Left) || keys.contains(&Keycode::A) {
            self.state &= (1 << 1) ^ 0xFF;
        } else {
            self.state |= 1 << 1;
        }

        if keys.contains(&Keycode::Up) || keys.contains(&Keycode::W) {
            self.state &= (1 << 2) ^ 0xFF;
        } else {
            self.state |= 1 << 2;
        }

        if keys.contains(&Keycode::Down) || keys.contains(&Keycode::S) {
            self.state &= (1 << 3) ^ 0xFF;
        } else {
            self.state |= 1 << 3;
        }

        if keys.contains(&Keycode::Enter) {
            println!("Enter");
            self.state &= (1 << 4) ^ 0xFF;
        } else {
            self.state |= 1 << 4;
        }

        if keys.contains(&Keycode::Backspace) || keys.contains(&Keycode::Q) {
            self.state &= (1 << 5) ^ 0xFF;
        } else {
            self.state |= 1 << 5;
        }

        if keys.contains(&Keycode::E) {
            self.state &= (1 << 6) ^ 0xFF;
        } else {
            self.state |= 1 << 6;
        }

        if keys.contains(&Keycode::Space) {
            self.state &= (1 << 7) ^ 0xFF;
        } else {
            self.state |= 1 << 7;
        }

        if prev_state == 0xFF && self.state != 0xFF {
            memory.request_interrupt(Interrupt::Joypad);
        }
    }

    pub fn tick(&mut self, nticks: u8, memory: &mut AddressSpace) {
        self.ticks += nticks as u64;
        if self.ticks >= 7022 {
            println!("Joy tick");
            self.ticks = self.ticks % 7022;
            self.update_state(memory);
            self.update_memory(memory);
        }
    }
}