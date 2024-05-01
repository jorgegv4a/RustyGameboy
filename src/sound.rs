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

const AUDIO_BUFFER_NUM_SAMPLES: usize = 512;
const TARGET_SAMPLE_RATE: usize = 44100;
const TIME_BETWEEN_BUFFERS: f64 = AUDIO_BUFFER_NUM_SAMPLES as f64 * 1.0 / TARGET_SAMPLE_RATE as f64;

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
    resample_frac: f32,
    freq_sweep_i: u8,
    freq_sweep_pace: u8,
    initial_volume: u8,
    shadow_period: u16,
    capacity: f32,
    negative_sweep_calc_executed: bool,
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
            resample_frac: 0.0,
            freq_sweep_i: 8,
            freq_sweep_pace: 0,
            initial_volume: 0,
            shadow_period: 1023,
            capacity: 0.0,
            negative_sweep_calc_executed: false,
        }
    }

    pub fn buffer_to_analog(&mut self, value: u8) -> f32 {
        if value == 255 {
            let v = 0.0;
            let wave_analog = v - self.capacity;
            self.capacity = v - wave_analog * 0.996;
            wave_analog
        } else {
            let v = (value  as f32 / -7.5) + 1.0;
            let wave_analog = v - self.capacity;
            self.capacity = v - wave_analog * 0.996;
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
    clock: u64,
    out_samples: Sender<[f32; 2*AUDIO_BUFFER_NUM_SAMPLES]>,
    // out_samples: SyncSender<[u8; AUDIO_BUFFER_NUM_SAMPLES]>,
    buffer: [u8; AUDIO_BUFFER_NUM_SAMPLES],
    buffer_i: usize,
    debug_file: File,
    debug_file2: File,
    debug_file3: File,
    debug_file4: File,
    frame_sequencer_i: u8,
    start_time: std::time::Instant,
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

        let filename = std::path::Path::new("CH3_out_pkmred.txt");
        if filename.exists() {
            fs::remove_file(filename);
        }
        let mut file = OpenOptions::new().create_new(true).write(true).append(true).open(filename).unwrap();

        let filename2 = std::path::Path::new("CH3_out_pkmred_vol.txt");
        if filename2.exists() {
            fs::remove_file(filename2);
        }
        let mut file2 = OpenOptions::new().create_new(true).write(true).append(true).open(filename2).unwrap();

        let filename3 = std::path::Path::new("CH3_out_pkmred_period.txt");
        if filename3.exists() {
            fs::remove_file(filename3);
        }
        let mut file3 = OpenOptions::new().create_new(true).write(true).append(true).open(filename3).unwrap();

        let filename4 = std::path::Path::new("CH3_out_pkmred_wave_duty.txt");
        if filename4.exists() {
            fs::remove_file(filename4);
        }
        let mut file4 = OpenOptions::new().create_new(true).write(true).append(true).open(filename4).unwrap();

        APU {
            audio_subsystem,
            div: None,
            div_apu: 0,
            device,
            ch1: Channel::new(),
            ch2: Channel::new(),
            ch3: Channel::new(),
            clock: 0,
            out_samples: tx,
            buffer: [0; AUDIO_BUFFER_NUM_SAMPLES],
            buffer_i: 0,
            debug_file: file,
            debug_file2: file2,
            debug_file3: file3,
            debug_file4: file4,
            frame_sequencer_i: 0,
            start_time: std::time::Instant::now(),
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
            println!("Ch1 off from period sweep (trigger, {} >> {})", self.ch1.shadow_period, ch1_period_sweep_step(memory));
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
            println!("Ch1 off from period sweep direction override");
            set_ch1_on(self.ch1.on, memory);
        }

        let new_freq = self.ch1_calc_new_freq(memory);
        if new_freq < 2048 && ch1_period_sweep_step(memory) > 0 {
            if new_freq != self.ch1.period {
                println!("[{:?}] New period update, {} -> {new_freq}", self.start_time.elapsed().as_secs_f32(), self.ch1.period);
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
                println!("Volume decreased from {}", self.ch1.volume);
                self.ch1.volume -= 1;
            }
        } else if ch1_envelope_direction(memory) == EnvelopeDirection::Inc {
            if self.ch1.volume < 15 {
                println!("Volume increased from {}", self.ch1.volume);
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
                println!("Volume decreased from {}", self.ch2.volume);
                self.ch2.volume -= 1;
            }
        } else if ch2_envelope_direction(memory) == EnvelopeDirection::Inc {
            if self.ch2.volume < 15 {
                println!("Volume increased from {}", self.ch2.volume);
                self.ch2.volume += 1;
            }
        }
    }

    fn ch1_length_tick(&mut self, memory: &mut AddressSpace) {
        if !self.ch1.length_enabled {
            return
        }
        self.ch1.length_i -= 1;
        println!("Length step: {}", self.ch1.length_i);
        if self.ch1.length_i != 0 {
            return
        }

        self.ch1.length_enabled = false;
        memory.write(NR14_ADDR, memory.apu_read(NR14_ADDR) & 0x87);

        self.ch1.on = false;
        set_ch1_on(self.ch1.on, memory);
        println!("Length expired!");
    }

    fn ch2_length_tick(&mut self, memory: &mut AddressSpace) {
        if !self.ch2.length_enabled {
            return
        }
        self.ch2.length_i -= 1;
        println!("Length step: {}", self.ch2.length_i);
        if self.ch2.length_i != 0 {
            return
        }

        self.ch2.length_enabled = false;
        memory.write(NR24_ADDR, memory.apu_read(NR24_ADDR) & 0x87);

        self.ch2.on = false;
        set_ch2_on(self.ch2.on, memory);
        println!("Length expired!");
    }

    fn ch3_length_tick(&mut self, memory: &mut AddressSpace) {
        if !self.ch3.length_enabled {
            return
        }

        self.ch3.length_i -= 1;
        println!("Length step: {}", self.ch3.length_i);
        if self.ch3.length_i != 0 {
            return
        }

        self.ch3.length_enabled = false;
        memory.write(NR34_ADDR, memory.apu_read(NR34_ADDR) & 0x87);

        self.ch3.on = false;
        set_ch3_on(self.ch3.on, memory);
        println!("Length expired!");
    }

    fn single_tick(&mut self, memory: &mut AddressSpace) {
        if self.frame_sequencer_i % 2 == 0 {
            self.ch1_length_tick(memory);
            self.ch2_length_tick(memory);
            self.ch3_length_tick(memory);
        } 
        if self.frame_sequencer_i % 4 == 2 {
            self.ch1_period_sweep_tick(memory);
        } 
        if self.frame_sequencer_i % 8 == 7 {
            self.ch1_envelope_tick(memory);
            self.ch2_envelope_tick(memory);
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

    pub fn ch1_tick(&mut self, memory: &mut AddressSpace) {
        if self.ch1.negative_sweep_calc_executed && ch1_sweep_direction(memory) == SweepDirection::Inc {
            self.ch1.on = false;
            set_ch1_on(self.ch1.on, memory);
        }
        if ch1_dac_on(memory) {
            if ch1_trigger(memory) {        
                let period_low = ch1_initial_period_low(memory);
                let period_high = ch1_initial_period_high(memory);
                self.ch1.period = ((period_high as u16) << 8) + period_low as u16;
                self.ch1.shadow_period = self.ch1.period;
                self.ch1.period_step = self.ch1.period;

                self.ch1.freq_sweep_pace = ch1_period_sweep_pace(memory);
                let period_sweep_shift = ch1_period_sweep_step(memory);
                self.ch1.freq_sweep_enabled = self.ch1.freq_sweep_pace > 0 || period_sweep_shift > 0;
                if self.ch1.freq_sweep_pace == 0 {
                    self.ch1.freq_sweep_pace = 8;
                }
                self.ch1.freq_sweep_i = self.ch1.freq_sweep_pace;
                self.ch1.negative_sweep_calc_executed = false;

                println!("Ch1 trigger!");
                self.ch1.on = true;
                self.ch1.initial_volume = ch1_initial_volume(memory);
                self.ch1.env_sweep_pace = ch1_envelope_sweep_pace(memory);
                self.ch1.env_sweep_step = self.ch1.env_sweep_pace;
                self.ch1.volume = self.ch1.initial_volume;

                if self.ch1.length_i == 0 {
                    self.ch1.length_i = 64;
                }

                if self.ch1.freq_sweep_enabled && period_sweep_shift > 0 {
                    self.ch1_calc_new_freq(memory);
                }
            }
        } else {
            self.ch1.on = false;
            println!("Ch1 DAC set to off");
        }
        set_ch1_on(self.ch1.on, memory);

        let wave_value;

        let length_timer_value = ch1_initial_len(memory);

        if self.ch1.on {
            if !self.ch1.length_enabled && ch1_length_enable(memory) {
                self.ch1.length_i = 64 - length_timer_value;
                println!("!!!!!!!!!!!!!!!!!!!!!!!Length enabled!");
                self.ch1.length_enabled = ch1_length_enable(memory);
            } else if length_timer_value > 0 {
                self.ch1.length_i = 64 - length_timer_value;
            }

            wave_value = self.ch1_get_next_sample(memory);
        } else {
            wave_value = 255;
        }

        // Fill buffer with raw sample rate, when enough samples collected resample to 44100
        let mut sampling_ratio = TARGET_SAMPLE_RATE as f32 / 1048576.0;
        self.ch1.resample_frac += sampling_ratio.fract();
        if self.ch1.resample_frac >= 1.0 {
            self.ch1.resample_frac -= 1.0;
            sampling_ratio += 1.0;
            println!("[{:?}] Generating 1 sample of period {} (value {wave_value}, vol: {})", self.start_time.elapsed().as_secs_f32(), self.ch1.period, self.ch1.volume);
        // }
        // let num_rep_samples = sampling_ratio.floor() as u32;
        // if num_rep_samples > 0 {
        //     println!("[{:?}]  Generating {num_rep_samples} samples of period {period}", self.start_time.elapsed().as_secs_f32());
        // }

        // for _ in 0..num_rep_samples {
            // self.debug_file2.write_all(&(self.ch1.volume as f32 / 15.0).to_be_bytes());
            // self.debug_file3.write_all(&(self.ch1.period as f32 / 2048.0).to_be_bytes());
            // match ch1_wave_duty(memory) {
            //     WaveDuty::Eight => self.debug_file4.write_all(&(1.0f32 / 8.0).to_be_bytes()),
            //     WaveDuty::Quarter => self.debug_file4.write_all(&(1.0f32 / 4.0).to_be_bytes()),
            //     WaveDuty::Half => self.debug_file4.write_all(&(1.0f32 / 2.0).to_be_bytes()),
            //     WaveDuty::InvQuarter => self.debug_file4.write_all(&(3.0f32 / 4.0).to_be_bytes()),
            // };
            if wave_value == 255 {
                self.buffer[self.buffer_i] = wave_value;
            } else {
                self.buffer[self.buffer_i] = wave_value * self.ch1.volume;
            }
            self.buffer_i += 1;
            if self.buffer_i == self.buffer.len() {
                self.buffer_i = 0;
                let mut analog_buffer = [0f32; 2*AUDIO_BUFFER_NUM_SAMPLES];
                for (i, x) in self.buffer.iter().enumerate() {
                    let wave_analog = self.ch1.buffer_to_analog(*x);
                    if ch1_pan_right(memory) {
                        analog_buffer[2 * i + 1] = wave_analog * ((master_right_volume(memory) + 1) as f32 / 8.0);
                    } else {
                        analog_buffer[2 * i + 1] = 0.0;
                    }
                    if ch1_pan_left(memory) {
                        analog_buffer[2* i] = wave_analog * ((master_left_volume(memory) + 1) as f32 / 8.0);
                    } else {
                        analog_buffer[2 * i] = 0.0;
                    }
                    self.debug_file.write_all(&analog_buffer[2* i].to_be_bytes());
                }
                let outcome = self.out_samples.send(analog_buffer);
                // match outcome {
                //     Ok(x) => println!("Send {}", wave_value),
                //     Err(s) => println!("Send Error: {s}"),
                // };
            }
        }
    }

    pub fn ch2_tick(&mut self, memory: &mut AddressSpace) {
        if ch2_dac_on(memory) {
            if ch2_trigger(memory) {        
                let period_low = ch2_initial_period_low(memory);
                let period_high = ch2_initial_period_high(memory);
                self.ch2.period = ((period_high as u16) << 8) + period_low as u16;
                self.ch2.period_step = self.ch2.period;

                println!("Ch2 trigger!");
                self.ch2.on = true;
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
            println!("Ch2 DAC set to off");
        }
        set_ch2_on(self.ch2.on, memory);

        let wave_value;

        let length_timer_value = ch2_initial_len(memory);

        if self.ch2.on {
            if !self.ch2.length_enabled && ch2_length_enable(memory) {
                self.ch2.length_i = 64 - length_timer_value;
                println!("!!!!!!!!!!!!!!!!!!!!!!!Length enabled!");
                self.ch2.length_enabled = ch2_length_enable(memory);
            } else if length_timer_value > 0 {
                self.ch2.length_i = 64 - length_timer_value;
            }

            wave_value = self.ch2_get_next_sample(memory);
        } else {
            wave_value = 255;
        }

        // Fill buffer with raw sample rate, when enough samples collected resample to 44100
        let mut sampling_ratio = TARGET_SAMPLE_RATE as f32 / 1048576.0;
        self.ch2.resample_frac += sampling_ratio.fract();
        if self.ch2.resample_frac >= 1.0 {
            self.ch2.resample_frac -= 1.0;
            sampling_ratio += 1.0;
            println!("[{:?}] Generating 1 sample of period {} (value {wave_value}, vol: {})", self.start_time.elapsed().as_secs_f32(), self.ch2.period, self.ch2.volume);
        // }
        // let num_rep_samples = sampling_ratio.floor() as u32;
        // if num_rep_samples > 0 {
        //     println!("[{:?}]  Generating {num_rep_samples} samples of period {period}", self.start_time.elapsed().as_secs_f32());
        // }

        // for _ in 0..num_rep_samples {
            // self.debug_file2.write_all(&(self.ch2.volume as f32 / 15.0).to_be_bytes());
            // self.debug_file3.write_all(&(self.ch2.period as f32 / 2048.0).to_be_bytes());
            // match ch2_wave_duty(memory) {
            //     WaveDuty::Eight => self.debug_file4.write_all(&(1.0f32 / 8.0).to_be_bytes()),
            //     WaveDuty::Quarter => self.debug_file4.write_all(&(1.0f32 / 4.0).to_be_bytes()),
            //     WaveDuty::Half => self.debug_file4.write_all(&(1.0f32 / 2.0).to_be_bytes()),
            //     WaveDuty::InvQuarter => self.debug_file4.write_all(&(3.0f32 / 4.0).to_be_bytes()),
            // };
            if wave_value == 255 {
                self.buffer[self.buffer_i] = wave_value;
            } else {
                self.buffer[self.buffer_i] = wave_value * self.ch2.volume;
            }
            self.buffer_i += 1;
            if self.buffer_i == self.buffer.len() {
                self.buffer_i = 0;
                let mut analog_buffer = [0f32; 2*AUDIO_BUFFER_NUM_SAMPLES];
                for (i, x) in self.buffer.iter().enumerate() {
                    let wave_analog = self.ch2.buffer_to_analog(*x);
                    if ch2_pan_right(memory) {
                        analog_buffer[2 * i + 1] = wave_analog * ((master_right_volume(memory) + 1) as f32 / 8.0);
                    } else {
                        analog_buffer[2 * i + 1] = 0.0;
                    }
                    if ch2_pan_left(memory) {
                        analog_buffer[2* i] = wave_analog * ((master_left_volume(memory) + 1) as f32 / 8.0);
                    } else {
                        analog_buffer[2 * i] = 0.0;
                    }
                    self.debug_file.write_all(&analog_buffer[2* i].to_be_bytes());
                }
                let outcome = self.out_samples.send(analog_buffer);
                // match outcome {
                //     Ok(x) => println!("Send {}", wave_value),
                //     Err(s) => println!("Send Error: {s}"),
                // };
            }
        }
    }


    pub fn ch3_tick(&mut self, memory: &mut AddressSpace) {
        if ch3_dac_on(memory) {
            if !self.ch3.on && ch3_trigger(memory) {
                self.ch3.on = true;
            }
        } else {
            self.ch3.on = false;
        }
        set_ch3_on(self.ch3.on, memory);

        let wave_value;

        if self.ch3.on {
            // println!("ON");
            if ch3_length_enable(memory) && !self.ch3.length_enabled {
                self.ch3.length_enabled = true;
                self.ch3.length_i = 255 - ch3_initial_len(memory);
                println!("Length enabled!");
            }
            
            wave_value = self.ch3_get_next_sample(memory);
        } else {
            wave_value = 255;
        }

        // println!("period: {period}");

        // Fill buffer with raw sample rate, when enough samples collected resample to 44100
        let mut sampling_ratio = TARGET_SAMPLE_RATE as f32 / 2097152.0;
        self.ch3.resample_frac += sampling_ratio.fract();
        if self.ch3.resample_frac >= 1.0 {
            self.ch3.resample_frac -= 1.0;
            sampling_ratio += 1.0;
        // }
        // let num_rep_samples = sampling_ratio.floor() as u32;

        // for _ in 0..num_rep_samples {
            self.buffer[self.buffer_i] = wave_value;
            self.buffer_i += 1;
            if self.buffer_i == self.buffer.len() {
                self.buffer_i = 0;
                let mut analog_buffer = [0f32; 2*AUDIO_BUFFER_NUM_SAMPLES];
                for (i, x) in self.buffer.iter().enumerate() {
                    let wave_analog = self.ch3.buffer_to_analog(*x);
                    if ch3_pan_right(memory) {
                        analog_buffer[2 * i + 1] = wave_analog * ((master_right_volume(memory) + 1) as f32 / 8.0);
                    } else {
                        analog_buffer[2 * i + 1] = 0.0;
                    }
                    if ch3_pan_left(memory) {
                        analog_buffer[2* i] = wave_analog * ((master_left_volume(memory) + 1) as f32 / 8.0);
                    } else {
                        analog_buffer[2 * i] = 0.0;
                    }
                    // self.debug_file.write_all(&analog_buffer[2* i].to_be_bytes());
                }
                let outcome = self.out_samples.send(analog_buffer);
                // match outcome {
                //     Ok(x) => println!("Send {}", wave_value),
                //     Err(s) => println!("Send Error: {s}"),
                // };
            }
        }
    }

    pub fn tick(&mut self, nticks: u8, memory: &mut AddressSpace) {
        // println!("Ticking APU with {nticks} ticks");
        let device_status = self.device.status();
        if !apu_enabled(memory) {
            if device_status == AudioStatus::Playing {
                // Pause playback
                self.device.pause();
                println!("APU DISABLED");
                // println!("Playback paused");
            }
            return;
        } else {
            if apu_enabled(memory) && device_status != AudioStatus::Playing {
                // Start playback
                self.device.resume();
                self.frame_sequencer_i = 0;
                println!("APU ENABLED");
                // println!("Playback enabled");
            }
        }
        for i in 0..nticks {
            // let div = (memory.read(DIV_ADDR) >> 4) & 1;
            let div = (self.clock >> (4 + 8)) & 1;
            if div == 0 && self.div.unwrap_or(0) == 1 {
                self.div_apu += 1; // ticks at 512 Hz
                self.single_tick(memory);
            }

            // if self.clock % 2 == 0 {
            //     self.ch3_tick(memory);
            // }
            if self.clock % 4 == 0 {
                // self.ch1_tick(memory);
                self.ch2_tick(memory);
            }
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

pub fn ch1_sweep_direction(memory: &AddressSpace) -> SweepDirection {
    if (memory.read(NR10_ADDR) >> 3) & 1 == 1 {
        SweepDirection::Dec
    } else {
        SweepDirection::Inc
    }
}

pub fn ch1_period_sweep_step(memory: &AddressSpace) -> u8 {
    return memory.read(NR10_ADDR) & 7;
}

pub fn ch1_wave_duty(memory: &AddressSpace) -> WaveDuty {
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

pub fn ch1_envelope_direction(memory: &AddressSpace) -> EnvelopeDirection {
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
pub fn ch2_wave_duty(memory: &AddressSpace) -> WaveDuty {
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

pub fn ch2_envelope_direction(memory: &AddressSpace) -> EnvelopeDirection {
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

pub fn ch3_output_level(memory: &AddressSpace) -> OutputLevel {
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

pub fn ch3_trigger(memory: &AddressSpace) -> bool {
    return (memory.apu_read(NR34_ADDR) >> 7) & 1 == 1;
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

pub fn ch4_envelope_direction(memory: &AddressSpace) -> EnvelopeDirection {
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

pub fn ch4_lfsr_width(memory: &AddressSpace) -> LFSRWidth {
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

pub fn ch4_trigger(memory: &AddressSpace) -> bool {
    return (memory.apu_read(NR44_ADDR) >> 7) & 1 == 1;
}