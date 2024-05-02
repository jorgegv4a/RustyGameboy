extern crate sdl2;

use sdl2::libc::sleep;
use sdl2::AudioSubsystem;
use sdl2::audio::{AudioCallback, AudioDevice, AudioQueue, AudioSpecDesired, AudioStatus};
use std::sync::mpsc;
use std::sync::mpsc::{Sender, Receiver, SyncSender};


use std::fs::{self, OpenOptions, File};
use std::io::prelude::*;
use std::time::Duration;

use crate::constants::*;
use crate::memory::AddressSpace;

#[derive(PartialEq)]
enum SweepDirection {
    Inc,
    Dec,
}
enum WaveDuty {
    Half,
    Quarter,
    Eight,
    InvQuarter,
}

#[derive(PartialEq)]
enum EnvelopeDirection {
    Inc,
    Dec,
}

enum OutputLevel {
    Mute,
    Full,
    Half,
    Quarter,
}

#[derive(PartialEq)]
enum LFSRWidth {
    Bits15,
    Bits7,
}

struct AudioPlayer {
    in_samples: Receiver<[f32; 2*AUDIO_BUFFER_NUM_SAMPLES]>,
    capacity: f32,
}

impl AudioCallback for AudioPlayer {
    type Channel = f32;

    fn callback(&mut self, out: &mut [f32]) {
        match self.in_samples.recv_timeout(std::time::Duration::from_secs_f32(0.030)) {
            Ok(buffer) => {
                for (i, x) in out.iter_mut().enumerate() {
                    *x = buffer[i];
                }
            },
            Err(s) => {
                println!("Failed to receive");
                for (i, x) in out.iter_mut().enumerate() {
                    *x = 0.0;
                }
            },
        }
    }
}


pub struct Channel {
    on: bool,
    volume: u8,
    env_sweep_step: u8,
    env_sweep_pace: u8,
    freq_sweep_enabled: bool,
    length_i: u8,
    length_enabled: bool,
    sample_index: u8,
    period_step: u16,
    period: u16,
    freq_sweep_i: u8,
    freq_sweep_pace: u8,
    initial_volume: u8,
    shadow_period: u16,
    capacity: f32,
    negative_sweep_calc_executed: bool,
    lfsr: u16,

}

impl Channel {
    pub fn new() -> Channel {
        Channel {
            on: false,
            volume: 0,
            env_sweep_step: 0,
            env_sweep_pace: 0,
            freq_sweep_enabled: false,
            length_i: 64,
            length_enabled: false,
            sample_index: 0,
            period_step: 1023,
            period: 1024,
            freq_sweep_i: 8,
            freq_sweep_pace: 0,
            initial_volume: 0,
            shadow_period: 1023,
            capacity: 0.0,
            negative_sweep_calc_executed: false,
            lfsr: 0,
        }
    }

    pub fn buffer_to_analog(&mut self, value: u8) -> f32 {
        if value == 255 {
            let v = 0.0;
            let wave_analog = v - self.capacity;
            self.capacity = v - wave_analog * HPF_CAPACITOR_CHARGE;
            wave_analog
        } else {
            let v = (value  as f32 / -7.5) + 1.0;
            let wave_analog = v - self.capacity;
            self.capacity = v - wave_analog * HPF_CAPACITOR_CHARGE;
            wave_analog
        }
    }
}

pub struct APU {
    audio_subsystem: AudioSubsystem,
    div: Option<u8>,
    div_apu: u64,
    device: AudioDevice<AudioPlayer>,
    ch1: Channel,
    ch2: Channel,
    ch3: Channel,
    ch4: Channel,
    clock: u64,
    out_samples: Sender<[f32; 2*AUDIO_BUFFER_NUM_SAMPLES]>,
    // out_samples: SyncSender<[u8; AUDIO_BUFFER_NUM_SAMPLES]>,
    buffer: [f32; 2 * AUDIO_BUFFER_NUM_SAMPLES],
    buffer_i: usize,
    frame_sequencer_i: u8,
    start_time: std::time::Instant,
    last_ch1_sample: u8,
    last_ch2_sample: u8,
    last_ch3_sample: u8,
    last_ch4_sample: u8,
    resample_frac: f32,
}


impl APU {
    pub fn new(audio_subsystem: AudioSubsystem) -> APU {
        let desired_spec = AudioSpecDesired {
            freq: Some(TARGET_SAMPLE_RATE as i32),
            channels: Some(2),  // stereo
            samples: Some(AUDIO_BUFFER_NUM_SAMPLES as u16),       // default sample size
        };

        let (tx, rx): (Sender<[f32; 2*AUDIO_BUFFER_NUM_SAMPLES]>, Receiver<[f32; 2*AUDIO_BUFFER_NUM_SAMPLES]>) = mpsc::channel();
        
        let device = audio_subsystem.open_playback(None, &desired_spec, |spec| {
            AudioPlayer {
                in_samples: rx,
                capacity: 0.0,
            }
        }).unwrap();

        APU {
            audio_subsystem,
            div: None,
            div_apu: 0,
            device,
            ch1: Channel::new(),
            ch2: Channel::new(),
            ch3: Channel::new(),
            ch4: Channel::new(),
            clock: 0,
            out_samples: tx,
            buffer: [0f32; 2 * AUDIO_BUFFER_NUM_SAMPLES],
            buffer_i: 0,
            frame_sequencer_i: 0,
            start_time: std::time::Instant::now(),
            last_ch1_sample: 255,
            last_ch2_sample: 255,
            last_ch3_sample: 255,
            last_ch4_sample: 255,
            resample_frac: 0.0,
        }
    }

    fn ch1_calc_new_freq(&mut self, memory: &mut AddressSpace) -> u16 {
        let mut new_freq = self.ch1.shadow_period >> ch1_period_sweep_step(memory);
        if ch1_sweep_direction(memory) == SweepDirection::Dec {
            new_freq = self.ch1.shadow_period - new_freq;
            self.ch1.negative_sweep_calc_executed = true;
        } else {
            new_freq = self.ch1.shadow_period + new_freq
        };
        if new_freq >= 2048 {
            self.ch1.on = false;
            set_ch1_on(self.ch1.on, memory);
        }
        return new_freq

    }

    fn ch1_period_sweep_tick(&mut self, memory: &mut AddressSpace) {
        if self.ch1.freq_sweep_pace == 0 {
            return
        }

        if self.ch1.freq_sweep_i > 0 {
            self.ch1.freq_sweep_i -= 1;
            if self.ch1.freq_sweep_i > 0 {
                return
            }
        }

        if !self.ch1.freq_sweep_enabled {
            return
        }
        

        self.ch1.freq_sweep_pace = ch1_period_sweep_pace(memory);
        self.ch1.freq_sweep_i = self.ch1.freq_sweep_pace;
        
        if self.ch1.freq_sweep_pace == 0 {
            self.ch1.freq_sweep_pace = 8;
            self.ch1.freq_sweep_i = self.ch1.freq_sweep_pace;
            return
        }

        if self.ch1.negative_sweep_calc_executed && ch1_sweep_direction(memory) == SweepDirection::Inc {
            self.ch1.on = false;
            set_ch1_on(self.ch1.on, memory);
        }

        let new_freq = self.ch1_calc_new_freq(memory);
        if new_freq < 2048 && ch1_period_sweep_step(memory) > 0 {
            if new_freq != self.ch1.period {
            }
            self.ch1.period = new_freq;
            self.ch1.shadow_period = new_freq;
            memory.apu_write_ch1_period(new_freq);
            self.ch1_calc_new_freq(memory);
        }
        
        
    }

    fn ch1_envelope_tick(&mut self, memory: &AddressSpace) {
        if self.ch1.env_sweep_pace == 0 {
            return
        }

        self.ch1.env_sweep_step -= 1;
        if self.ch1.env_sweep_step > 0 {
            return
        }

        self.ch1.env_sweep_step = self.ch1.env_sweep_pace;

        if ch1_envelope_direction(memory) == EnvelopeDirection::Dec {
            if self.ch1.volume > 0 {
                self.ch1.volume -= 1;
            }
        } else if ch1_envelope_direction(memory) == EnvelopeDirection::Inc {
            if self.ch1.volume < 15 {
                self.ch1.volume += 1;
            }
        }
    }

    fn ch2_envelope_tick(&mut self, memory: &AddressSpace) {
        if self.ch2.env_sweep_pace == 0 {
            return
        }

        self.ch2.env_sweep_step -= 1;
        if self.ch2.env_sweep_step > 0 {
            return
        }

        self.ch2.env_sweep_step = self.ch2.env_sweep_pace;

        if ch2_envelope_direction(memory) == EnvelopeDirection::Dec {
            if self.ch2.volume > 0 {
                self.ch2.volume -= 1;
            }
        } else if ch2_envelope_direction(memory) == EnvelopeDirection::Inc {
            if self.ch2.volume < 15 {
                self.ch2.volume += 1;
            }
        }
    }

    fn ch4_envelope_tick(&mut self, memory: &AddressSpace) {
        if self.ch4.env_sweep_pace == 0 {
            return
        }

        self.ch4.env_sweep_step -= 1;
        if self.ch4.env_sweep_step > 0 {
            return
        }

        self.ch4.env_sweep_step = self.ch4.env_sweep_pace;

        if ch4_envelope_direction(memory) == EnvelopeDirection::Dec {
            if self.ch4.volume > 0 {
                self.ch4.volume -= 1;
            }
        } else if ch4_envelope_direction(memory) == EnvelopeDirection::Inc {
            if self.ch4.volume < 15 {
                self.ch4.volume += 1;
            }
        }
    }

    fn ch1_length_tick(&mut self, memory: &mut AddressSpace) {
        if !self.ch1.length_enabled {
            return
        }
        self.ch1.length_i -= 1;
        if self.ch1.length_i != 0 {
            return
        }

        self.ch1.length_enabled = false;
        memory.write(NR14_ADDR, memory.apu_read(NR14_ADDR) & 0x87);

        self.ch1.on = false;
        set_ch1_on(self.ch1.on, memory);
    }

    fn ch2_length_tick(&mut self, memory: &mut AddressSpace) {
        if !self.ch2.length_enabled {
            return
        }
        self.ch2.length_i -= 1;
        if self.ch2.length_i != 0 {
            return
        }

        self.ch2.length_enabled = false;
        memory.write(NR24_ADDR, memory.apu_read(NR24_ADDR) & 0x87);

        self.ch2.on = false;
        set_ch2_on(self.ch2.on, memory);
    }

    fn ch3_length_tick(&mut self, memory: &mut AddressSpace) {
        if !self.ch3.length_enabled {
            return
        }

        self.ch3.length_i -= 1;
        if self.ch3.length_i != 0 {
            return
        }

        self.ch3.length_enabled = false;
        memory.write(NR34_ADDR, memory.apu_read(NR34_ADDR) & 0x87);

        self.ch3.on = false;
        set_ch3_on(self.ch3.on, memory);
    }

    fn ch4_length_tick(&mut self, memory: &mut AddressSpace) {
        if !self.ch4.length_enabled {
            return
        }
        self.ch4.length_i -= 1;
        if self.ch4.length_i != 0 {
            return
        }

        self.ch4.length_enabled = false;
        memory.write(NR44_ADDR, memory.apu_read(NR44_ADDR) & 0x87);

        self.ch4.on = false;
        set_ch4_on(self.ch4.on, memory);
    }

    fn single_tick(&mut self, memory: &mut AddressSpace) {
        if self.frame_sequencer_i % 2 == 0 {
            self.ch1_length_tick(memory);
            self.ch2_length_tick(memory);
            self.ch3_length_tick(memory);
            self.ch4_length_tick(memory);
        } 
        if self.frame_sequencer_i % 4 == 2 {
            self.ch1_period_sweep_tick(memory);
        } 
        if self.frame_sequencer_i % 8 == 7 {
            self.ch1_envelope_tick(memory);
            self.ch2_envelope_tick(memory);
            self.ch4_envelope_tick(memory);
        }
        self.frame_sequencer_i = (self.frame_sequencer_i + 1) % 8;
    }

    pub fn ch1_sample(&self, memory: &AddressSpace) -> u8 {
        if self.ch1.sample_index >= 8 {
            panic!("Invalid ch1 sample: {:02X}", self.ch1.sample_index);
        }
        match ch1_wave_duty(memory) {
            WaveDuty::Eight => [0, 0, 0, 0, 0, 0, 0, 1][self.ch1.sample_index as usize],
            WaveDuty::Quarter => [1, 0, 0, 0, 0, 0, 0, 1][self.ch1.sample_index as usize],
            WaveDuty::Half => [1, 0, 0, 0, 0, 1, 1, 1][self.ch1.sample_index as usize],
            WaveDuty::InvQuarter => [0, 1, 1, 1, 1, 1, 1, 0][self.ch1.sample_index as usize],
        }
    }

    fn ch1_get_next_sample(&mut self, memory: &AddressSpace) -> u8 {
        self.ch1.period_step += 1;
        if self.ch1.period_step >= 2048 {
            self.ch1.sample_index = (self.ch1.sample_index + 1) % 8;
            let period_low = ch1_initial_period_low(memory);
            let period_high = ch1_initial_period_high(memory);
            self.ch1.period = ((period_high as u16) << 8) + period_low as u16;
            self.ch1.period_step = self.ch1.period;
        }
        self.ch1_sample(memory)
    }

    pub fn ch2_sample(&self, memory: &AddressSpace) -> u8 {
        if self.ch2.sample_index >= 8 {
            panic!("Invalid ch2 sample: {:02X}", self.ch2.sample_index);
        }
        match ch2_wave_duty(memory) {
            WaveDuty::Eight => [0, 0, 0, 0, 0, 0, 0, 1][self.ch2.sample_index as usize],
            WaveDuty::Quarter => [1, 0, 0, 0, 0, 0, 0, 1][self.ch2.sample_index as usize],
            WaveDuty::Half => [1, 0, 0, 0, 0, 1, 1, 1][self.ch2.sample_index as usize],
            WaveDuty::InvQuarter => [0, 1, 1, 1, 1, 1, 1, 0][self.ch2.sample_index as usize],
        }
    }

    fn ch2_get_next_sample(&mut self, memory: &AddressSpace) -> u8 {
        self.ch2.period_step += 1;
        if self.ch2.period_step >= 2048 {
            self.ch2.sample_index = (self.ch2.sample_index + 1) % 8;
            let period_low = ch2_initial_period_low(memory);
            let period_high = ch2_initial_period_high(memory);
            self.ch2.period = ((period_high as u16) << 8) + period_low as u16;
            self.ch2.period_step = self.ch2.period;
        }
        self.ch2_sample(memory)
    }

    fn ch3_get_next_sample(&mut self, memory: &AddressSpace) -> u8 {
        self.ch3.period_step += 1;
        if self.ch3.period_step >= 2048 {
            self.ch3.sample_index = (self.ch3.sample_index + 1) % 32;
            self.ch3.period = ((ch3_initial_period_high(memory) as u16) << 8) + ch3_initial_period_low(memory) as u16;
            self.ch3.period_step = self.ch3.period;
        } 
        let wave_value = ch3_wave_sample(self.ch3.sample_index, memory);
        match ch3_output_level(memory) {
            OutputLevel::Full => wave_value,
            OutputLevel::Half => wave_value >> 1,
            OutputLevel::Quarter => wave_value >> 2,
            OutputLevel::Mute => 0,
        }
    }

    pub fn ch4_sample(&self) -> u8 {
        (self.ch4.lfsr & 1) as u8
    }

    fn ch4_get_next_sample(&mut self, memory: &AddressSpace) -> u8 {
        self.ch4.period_step -= 1;
        if self.ch4.period_step == 0 {
            let divisor_value = ch4_clock_divider(memory);
            let divisor = if divisor_value > 0 {
                divisor_value * 16
            } else {
                8
            };
            self.ch4.period_step = (divisor as u16) << ch4_clock_shift(memory);
            let value = ((self.ch4.lfsr & 1) ^ ((self.ch4.lfsr >> 1) & 1)) & 1;
            // self.ch4.lfsr &= (1 << 15) ^ 0xFFFF;
            self.ch4.lfsr = ((value ^ 1) << 14) | (self.ch4.lfsr >> 1);
            if ch4_lfsr_width(memory) == LFSRWidth::Bits7 {
                self.ch4.lfsr &= (1 << 6) ^ 0xFFFF;
                self.ch4.lfsr |= (value ^ 1) << 6;
            }
        }
        self.ch4_sample()
    }

    pub fn ch1_tick(&mut self, memory: &mut AddressSpace) -> u8 {
        if self.ch1.negative_sweep_calc_executed && ch1_sweep_direction(memory) == SweepDirection::Inc {
            self.ch1.on = false;
            set_ch1_on(self.ch1.on, memory);
        }
        if ch1_dac_on(memory) {
            if ch1_trigger(memory) {
                self.ch1.on = true;
                // Period registers
                let period_low = ch1_initial_period_low(memory);
                let period_high = ch1_initial_period_high(memory);
                self.ch1.period = ((period_high as u16) << 8) + period_low as u16;
                self.ch1.shadow_period = self.ch1.period;
                self.ch1.period_step = self.ch1.period;

                // Freq Sweep registers
                self.ch1.freq_sweep_pace = ch1_period_sweep_pace(memory);
                let period_sweep_shift = ch1_period_sweep_step(memory);
                self.ch1.freq_sweep_enabled = self.ch1.freq_sweep_pace > 0 || period_sweep_shift > 0;
                if self.ch1.freq_sweep_pace == 0 {
                    self.ch1.freq_sweep_pace = 8;
                }
                self.ch1.freq_sweep_i = self.ch1.freq_sweep_pace;
                self.ch1.negative_sweep_calc_executed = false;
                if self.ch1.freq_sweep_enabled && period_sweep_shift > 0 {
                    self.ch1_calc_new_freq(memory);
                }

                // Envelope sweep registers
                self.ch1.initial_volume = ch1_initial_volume(memory);
                self.ch1.env_sweep_pace = ch1_envelope_sweep_pace(memory);
                self.ch1.env_sweep_step = self.ch1.env_sweep_pace;
                self.ch1.volume = self.ch1.initial_volume;

                if self.ch1.length_i == 0 {
                    self.ch1.length_i = 64;
                }
            }
        } else {
            self.ch1.on = false;
        }
        set_ch1_on(self.ch1.on, memory);

        let length_timer_value = ch1_initial_len(memory);

        if self.ch1.on {
            if !self.ch1.length_enabled && ch1_length_enable(memory) {
                self.ch1.length_i = 64 - length_timer_value;
                self.ch1.length_enabled = ch1_length_enable(memory);
            } else if length_timer_value > 0 {
                self.ch1.length_i = 64 - length_timer_value;
            }
            self.ch1_get_next_sample(memory) * self.ch1.volume
        } else {
            255
        }
    }

    pub fn ch2_tick(&mut self, memory: &mut AddressSpace) -> u8 {
        if ch2_dac_on(memory) {
            if ch2_trigger(memory) {     
                self.ch2.on = true;
                // Period registers   
                let period_low = ch2_initial_period_low(memory);
                let period_high = ch2_initial_period_high(memory);
                self.ch2.period = ((period_high as u16) << 8) + period_low as u16;
                self.ch2.period_step = self.ch2.period;

                // Envelope sweep registers
                self.ch2.initial_volume = ch2_initial_volume(memory);
                self.ch2.env_sweep_pace = ch2_envelope_sweep_pace(memory);
                self.ch2.env_sweep_step = self.ch2.env_sweep_pace;
                self.ch2.volume = self.ch2.initial_volume;

                if self.ch2.length_i == 0 {
                    self.ch2.length_i = 64;
                }
            }
        } else {
            self.ch2.on = false;
        }
        set_ch2_on(self.ch2.on, memory);

        let length_timer_value = ch2_initial_len(memory);

        if self.ch2.on {
            if !self.ch2.length_enabled && ch2_length_enable(memory) {
                self.ch2.length_i = 64 - length_timer_value;
                self.ch2.length_enabled = ch2_length_enable(memory);
            } else if length_timer_value > 0 {
                self.ch2.length_i = 64 - length_timer_value;
            }
            self.ch2_get_next_sample(memory) * self.ch2.volume
        } else {
            255
        }
    }


    pub fn ch3_tick(&mut self, memory: &mut AddressSpace) -> u8 {
        if ch3_dac_on(memory) {
            if ch3_trigger(memory) {
                self.ch3.on = true;
                // Period registers
                let period_low = ch3_initial_period_low(memory);
                let period_high = ch3_initial_period_high(memory);
                self.ch3.period = ((period_high as u16) << 8) + period_low as u16;
            }
        } else {
            self.ch3.on = false;
        }
        set_ch3_on(self.ch3.on, memory);

        if self.ch3.on {
            if ch3_length_enable(memory) && !self.ch3.length_enabled {
                self.ch3.length_enabled = true;
                self.ch3.length_i = 255 - ch3_initial_len(memory);
            }
            
            self.ch3_get_next_sample(memory)
        } else {
            255
        }
    }

    pub fn ch4_tick(&mut self, memory: &mut AddressSpace) -> u8 {
        if ch4_dac_on(memory) {
            if ch4_trigger(memory) {
                self.ch4.lfsr = 0;
                self.ch4.on = true;
                // Envelope sweep registers
                self.ch4.initial_volume = ch4_initial_volume(memory);
                self.ch4.env_sweep_pace = ch4_envelope_sweep_pace(memory);
                self.ch4.env_sweep_step = self.ch4.env_sweep_pace;
                self.ch4.volume = self.ch4.initial_volume;

                if self.ch4.length_i == 0 {
                    self.ch4.length_i = 64;
                }
            }
        } else {
            self.ch4.on = false;
        }
        set_ch4_on(self.ch4.on, memory);

        let length_timer_value = ch4_initial_len(memory);

        if self.ch4.on {
            if !self.ch4.length_enabled && ch4_length_enable(memory) {
                self.ch4.length_i = 64 - length_timer_value;
                self.ch4.length_enabled = ch4_length_enable(memory);
            } else if length_timer_value > 0 {
                self.ch4.length_i = 64 - length_timer_value;
            }
            self.ch4_get_next_sample(memory) * self.ch4.volume
        } else {
            255
        }
    }

    pub fn tick(&mut self, nticks: u8, memory: &mut AddressSpace) {
        let device_status = self.device.status();
        if !apu_enabled(memory) {
            if device_status == AudioStatus::Playing {
                // Pause playback
                self.device.pause();
            }
            return;
        } else {
            if apu_enabled(memory) && device_status != AudioStatus::Playing {
                // Start playback
                self.device.resume();
                self.frame_sequencer_i = 0;
            }
        }
        for i in 0..nticks {
            let div = (self.clock >> (4 + 8)) & 1;
            if div == 0 && self.div.unwrap_or(0) == 1 {
                self.div_apu += 1; // ticks at 512 Hz
                self.single_tick(memory);
            }

            let ch4_sample = self.ch4_tick(memory);
            self.last_ch4_sample = ch4_sample;

            if self.clock % 4 == 0 {
                let ch1_sample = self.ch1_tick(memory);
                let ch2_sample = self.ch2_tick(memory);
                self.last_ch1_sample = ch1_sample;
                self.last_ch2_sample = ch2_sample;
            }

            if self.clock % 2 == 0 {
                let ch3_sample = self.ch3_tick(memory);
                self.last_ch3_sample = ch3_sample;
            }

            self.resample_frac += (TARGET_SAMPLE_RATE as f32 / 4194304.0).fract();

            if self.resample_frac >= 1.0 {
                self.resample_frac -= 1.0;
                let ch1_analog = self.ch1.buffer_to_analog(self.last_ch1_sample);
                let ch2_analog = self.ch2.buffer_to_analog(self.last_ch2_sample);
                let ch3_analog = self.ch3.buffer_to_analog(self.last_ch3_sample);
                let ch4_analog = self.ch4.buffer_to_analog(self.last_ch4_sample);

                let mut left_analog: f32 = 0.0;
                let mut right_analog: f32 = 0.0;


                if ch1_pan_right(memory) {
                    right_analog += ch1_analog;
                }
                if ch1_pan_left(memory) {
                    left_analog += ch1_analog;
                }

                if ch2_pan_right(memory) {
                    right_analog += ch2_analog;
                }
                if ch2_pan_left(memory) {
                    left_analog += ch2_analog;
                }

                if ch3_pan_right(memory) {
                    right_analog += ch3_analog;
                }
                if ch3_pan_left(memory) {
                    left_analog += ch3_analog;
                }

                if ch4_pan_right(memory) {
                    right_analog += ch4_analog;
                }
                if ch4_pan_left(memory) {
                    left_analog += ch4_analog;
                }

                left_analog = (left_analog / 4.0) * ((master_left_volume(memory) + 1) as f32 / 8.0);
                right_analog = (right_analog / 4.0) * ((master_right_volume(memory) + 1) as f32 / 8.0);
                self.buffer[2 * self.buffer_i] = left_analog;
                self.buffer[2 * self.buffer_i + 1] = right_analog;

                // self.debug_file.write_all(&self.buffer[2 * self.buffer_i].to_be_bytes());

                self.buffer_i += 1;
                if self.buffer_i == AUDIO_BUFFER_NUM_SAMPLES {
                    self.buffer_i = 0;
                    let outcome = self.out_samples.send(self.buffer.clone());
                }
            }

            // TODO: continue mixing samples
            self.clock += 1;
            self.div = Some((div & 1) as u8);
        }
    }
}

pub fn apu_enabled(memory: &AddressSpace) -> bool {
    return (memory.read(NR52_ADDR) >> APU_ENABLE_BIT) & 1 == 1;
}


pub fn ch1_on(memory: &AddressSpace) -> bool {
    return (memory.read(NR52_ADDR) >> APU_CH1_ON_BIT) & 1 == 1;
}

pub fn ch2_on(memory: &AddressSpace) -> bool {
    return (memory.read(NR52_ADDR) >> APU_CH2_ON_BIT) & 1 == 1;
}

pub fn ch3_on(memory: &AddressSpace) -> bool {
    return (memory.read(NR52_ADDR) >> APU_CH3_ON_BIT) & 1 == 1;
}

pub fn ch4_on(memory: &AddressSpace) -> bool {
    return (memory.read(NR52_ADDR) >> APU_CH4_ON_BIT) & 1 == 1;
}

pub fn set_ch1_on(status: bool, memory: &mut AddressSpace) {
    let mut value = memory.apu_read(NR52_ADDR);
    if status {
        value |= 1 << APU_CH1_ON_BIT;
        memory.apu_write_nr52(value);
    } else {
        value &= (1 << APU_CH1_ON_BIT) ^ 0xFF;
        memory.apu_write_nr52(value);
    }
}

pub fn set_ch2_on(status: bool, memory: &mut AddressSpace) {
    let mut value = memory.apu_read(NR52_ADDR);
    if status {
        value |= 1 << APU_CH2_ON_BIT;
        memory.apu_write_nr52(value);
    } else {
        value &= (1 << APU_CH2_ON_BIT) ^ 0xFF;
        memory.apu_write_nr52(value);
    }
}

pub fn set_ch3_on(status: bool, memory: &mut AddressSpace) {
    let mut value = memory.apu_read(NR52_ADDR);
    if status {
        value |= 1 << APU_CH3_ON_BIT;
        memory.apu_write_nr52(value);
    } else {
        value &= (1 << APU_CH3_ON_BIT) ^ 0xFF;
        memory.apu_write_nr52(value);
    }
}

pub fn set_ch4_on(status: bool, memory: &mut AddressSpace) {
    let mut value = memory.apu_read(NR52_ADDR);
    if status {
        value |= 1 << APU_CH4_ON_BIT;
        memory.apu_write_nr52(value);
    } else {
        value &= (1 << APU_CH4_ON_BIT) ^ 0xFF;
        memory.apu_write_nr52(value);
    }
}

pub fn master_left_volume(memory: &AddressSpace) -> u8 {
    return (memory.read(NR50_ADDR) >> 4) & 7;
}

pub fn master_right_volume(memory: &AddressSpace) -> u8 {
    return memory.read(NR50_ADDR) & 7;
}

pub fn ch1_pan_left(memory: &AddressSpace) -> bool {
    return (memory.read(NR51_ADDR) >> APU_CH1_PAN_LEFT_BIT) & 1 == 1;
}

pub fn ch2_pan_left(memory: &AddressSpace) -> bool {
    return (memory.read(NR51_ADDR) >> APU_CH2_PAN_LEFT_BIT) & 1 == 1;
}

pub fn ch3_pan_left(memory: &AddressSpace) -> bool {
    return (memory.read(NR51_ADDR) >> APU_CH3_PAN_LEFT_BIT) & 1 == 1;
}

pub fn ch4_pan_left(memory: &AddressSpace) -> bool {
    return (memory.read(NR51_ADDR) >> APU_CH4_PAN_LEFT_BIT) & 1 == 1;
}

pub fn ch1_pan_right(memory: &AddressSpace) -> bool {
    return (memory.read(NR51_ADDR) >> APU_CH1_PAN_RIGHT_BIT) & 1 == 1;
}

pub fn ch2_pan_right(memory: &AddressSpace) -> bool {
    return (memory.read(NR51_ADDR) >> APU_CH2_PAN_RIGHT_BIT) & 1 == 1;
}

pub fn ch3_pan_right(memory: &AddressSpace) -> bool {
    return (memory.read(NR51_ADDR) >> APU_CH3_PAN_RIGHT_BIT) & 1 == 1;
}

pub fn ch4_pan_right(memory: &AddressSpace) -> bool {
    return (memory.read(NR51_ADDR) >> APU_CH4_PAN_RIGHT_BIT) & 1 == 1;
}

pub fn ch1_period_sweep_pace(memory: &AddressSpace) -> u8 {
    return (memory.read(NR10_ADDR) >> 4) & 7;
}

fn ch1_sweep_direction(memory: &AddressSpace) -> SweepDirection {
    if (memory.read(NR10_ADDR) >> 3) & 1 == 1 {
        SweepDirection::Dec
    } else {
        SweepDirection::Inc
    }
}

pub fn ch1_period_sweep_step(memory: &AddressSpace) -> u8 {
    return memory.read(NR10_ADDR) & 7;
}

fn ch1_wave_duty(memory: &AddressSpace) -> WaveDuty {
    match (memory.apu_read(NR11_ADDR) >> 6) & 3 {
        0 => WaveDuty::Eight,
        1 => WaveDuty::Quarter,
        2 => WaveDuty::Half,
        3 => WaveDuty::InvQuarter,
        x => unreachable!("Invalid value for ch1_wave_duty: {x}"),
    }
}

pub fn ch1_initial_len(memory: &mut AddressSpace) -> u8 {
    let value = memory.apu_read(NR11_ADDR);
    memory.write(NR11_ADDR, value & 0xC0);
    value & 0x3F
}

pub fn ch1_initial_volume(memory: &AddressSpace) -> u8 {
    return (memory.read(NR12_ADDR) >> 4) & 0xF;
}

fn ch1_envelope_direction(memory: &AddressSpace) -> EnvelopeDirection {
    if (memory.read(NR12_ADDR) >> 3) & 1 == 0 {
        EnvelopeDirection::Dec
    } else {
        EnvelopeDirection::Inc
    }
}

pub fn ch1_envelope_sweep_pace(memory: &AddressSpace) -> u8 {
    return memory.read(NR12_ADDR) & 0x7;
}

pub fn ch1_dac_on(memory: &AddressSpace) -> bool {
    return (memory.read(NR12_ADDR) >> 3) & 0x1F != 0;
}

pub fn ch1_initial_period_low(memory: &AddressSpace) -> u8 {
    let value = memory.apu_read(NR13_ADDR);
    // memory.write(NR13_ADDR, 0);
    value
}

pub fn ch1_initial_period_high(memory: &AddressSpace) -> u8 {
    let value = memory.apu_read(NR14_ADDR);
    // memory.write(NR14_ADDR, value & 0xF8);
    value & 0x7
}

pub fn ch1_length_enable(memory: &AddressSpace) -> bool {
    return (memory.read(NR14_ADDR) >> 6) & 1 == 1;
}

pub fn ch1_trigger(memory: &mut AddressSpace) -> bool {
    let reg_value = memory.apu_read(NR14_ADDR);
    let state = (reg_value >> 7) & 1 == 1;
    if state {
        memory.write(NR14_ADDR, reg_value & 0x4F)   
    };
    state
}

///
/// 
fn ch2_wave_duty(memory: &AddressSpace) -> WaveDuty {
    match (memory.apu_read(NR21_ADDR) >> 6) & 3 {
        0 => WaveDuty::Eight,
        1 => WaveDuty::Quarter,
        2 => WaveDuty::Half,
        3 => WaveDuty::InvQuarter,
        x => unreachable!("Invalid value for ch2_wave_duty: {x}"),
    }
}

pub fn ch2_initial_len(memory: &mut AddressSpace) -> u8 {
    let value = memory.apu_read(NR21_ADDR);
    memory.write(NR21_ADDR, value & 0xC0);
    value & 0x3F
}

pub fn ch2_initial_volume(memory: &AddressSpace) -> u8 {
    return (memory.read(NR22_ADDR) >> 4) & 0xF;
}

fn ch2_envelope_direction(memory: &AddressSpace) -> EnvelopeDirection {
    if (memory.read(NR22_ADDR) >> 3) & 1 == 0 {
        EnvelopeDirection::Dec
    } else {
        EnvelopeDirection::Inc
    }
}

pub fn ch2_envelope_sweep_pace(memory: &AddressSpace) -> u8 {
    return memory.read(NR22_ADDR) & 0x7;
}

pub fn ch2_dac_on(memory: &AddressSpace) -> bool {
    return (memory.read(NR22_ADDR) >> 3) & 0x1F != 0;
}

pub fn ch2_initial_period_low(memory: &AddressSpace) -> u8 {
    let value = memory.apu_read(NR23_ADDR);
    value
}

pub fn ch2_initial_period_high(memory: &AddressSpace) -> u8 {
    let value = memory.apu_read(NR24_ADDR);
    value & 0x7
}

pub fn ch2_length_enable(memory: &AddressSpace) -> bool {
    return (memory.read(NR24_ADDR) >> 6) & 1 == 1;
}

pub fn ch2_trigger(memory: &mut AddressSpace) -> bool {
    let reg_value = memory.apu_read(NR24_ADDR);
    let state = (reg_value >> 7) & 1 == 1;
    if state {
        memory.write(NR24_ADDR, reg_value & 0x4F)   
    };
    state
}

pub fn ch3_dac_on(memory: &AddressSpace) -> bool {
    return (memory.read(NR30_ADDR) >> 7) & 1 == 1;
}

pub fn ch3_initial_len(memory: &AddressSpace) -> u8 {
    return memory.apu_read(NR31_ADDR);
}

fn ch3_output_level(memory: &AddressSpace) -> OutputLevel {
    match (memory.read(NR32_ADDR) >> 5) & 0x3 {
        0 => OutputLevel::Mute,
        1 => OutputLevel::Full,
        2 => OutputLevel::Half,
        3 => OutputLevel::Quarter,
        x => unreachable!("Invalid value for ch3_output_level: {x}"),
    }
}

pub fn ch3_initial_period_low(memory: &AddressSpace) -> u8 {
    return memory.apu_read(NR33_ADDR);
}

pub fn ch3_initial_period_high(memory: &AddressSpace) -> u8 {
    return memory.apu_read(NR34_ADDR) & 0x7;
}

pub fn ch3_length_enable(memory: &AddressSpace) -> bool {
    return (memory.read(NR34_ADDR) >> 6) & 1 == 1;
}

pub fn ch3_trigger(memory: &mut AddressSpace) -> bool {
    let reg_value = memory.apu_read(NR34_ADDR);
    let state = (reg_value >> 7) & 1 == 1;
    if state {
        memory.write(NR34_ADDR, reg_value & 0x4F)   
    };
    state
}

pub fn ch3_wave_sample(index: u8, memory: &AddressSpace) -> u8 {
    if index >= 32 {
        panic!("Invalid ch3 wave sample: {index:02X}");
    }
    let item_byte = memory.read((WAVE_RANGE_START + index as u16/ 2));
    if index % 2 == 1 {
        item_byte & 0xF
    } else {
        (item_byte >> 4) & 0xF
    }
}

pub fn ch4_initial_len(memory: &AddressSpace) -> u8 {
    return memory.apu_read(NR41_ADDR) & 0x3F;
}

pub fn ch4_initial_volume(memory: &AddressSpace) -> u8 {
    return (memory.read(NR42_ADDR) >> 4) & 0xF;
}

fn ch4_envelope_direction(memory: &AddressSpace) -> EnvelopeDirection {
    if (memory.read(NR42_ADDR) >> 3) & 1 == 0 {
        EnvelopeDirection::Dec
    } else {
        EnvelopeDirection::Inc
    }
}

pub fn ch4_envelope_sweep_pace(memory: &AddressSpace) -> u8 {
    return memory.read(NR42_ADDR) & 0x7;
}

pub fn ch4_dac_on(memory: &AddressSpace) -> bool {
    return (memory.read(NR42_ADDR) >> 3) & 0x1F != 0;
}

pub fn ch4_clock_divider(memory: &AddressSpace) -> u8 {
    return memory.read(NR43_ADDR) & 0x7;
}

fn ch4_lfsr_width(memory: &AddressSpace) -> LFSRWidth {
    if (memory.read(NR43_ADDR) >> 3) & 0x1 == 1 {
        LFSRWidth::Bits7
    } else {
        LFSRWidth::Bits15
    }
}

pub fn ch4_clock_shift(memory: &AddressSpace) -> u8 {
    return (memory.read(NR43_ADDR) >> 4) & 0x7
}

pub fn ch4_length_enable(memory: &AddressSpace) -> bool {
    return (memory.read(NR44_ADDR) >> 6) & 1 == 1;
}

pub fn ch4_trigger(memory: &mut AddressSpace) -> bool {
    let reg_value = memory.apu_read(NR44_ADDR);
    let state = (reg_value >> 7) & 1 == 1;
    if state {
        memory.write(NR44_ADDR, reg_value & 0x4F)   
    };
    state
}