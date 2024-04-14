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

enum SweepDirection {
    Inc,
    Dec,
}
enum WaveDuty {
    Half,
    Quarter,
    Eight,
}
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

struct PulseWave {
    in_samples: Receiver<[f32; 2*AUDIO_BUFFER_NUM_SAMPLES]>,
    phase_inc: f32,
    phase: f32,
    volume: f32,
    pace: u8,
    step: u8,
    last_buffer_time: std::time::Instant,
}

impl AudioCallback for PulseWave {
    type Channel = f32;

    fn callback(&mut self, out: &mut [f32]) {
        match self.in_samples.recv_timeout(std::time::Duration::from_secs_f32(0.030)) {
            Ok(buffer) => {
                // println!("Callback recv");
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
    length: u8,
    length_i: u8,
    freq: u8,
    length_enabled: bool,
}

impl Channel {
    pub fn new() -> Channel {
        Channel {
            on: false,
            volume: 0,
            length: 0,
            length_i: 0,
            freq: 0,
            length_enabled: false,
        }
    }
}

pub struct APU {
    audio_subsystem: AudioSubsystem,
    latched_div: Option<u8>,
    div_apu: u64,
    device: AudioDevice<PulseWave>,
    // device: AudioQueue<f32>,
    latched_pace: Option<u8>,
    ch3: Channel,
    ch3_sample_index: u8,
    period_step: u32,
    latched_ch3_period: f32,
    clock: u64,
    out_samples: Sender<[f32; 2*AUDIO_BUFFER_NUM_SAMPLES]>,
    // out_samples: SyncSender<[u8; AUDIO_BUFFER_NUM_SAMPLES]>,
    buffer: [u8; AUDIO_BUFFER_NUM_SAMPLES],
    buffer_i: usize,
    debug_file: File,
    resample_frac: f32,
    last_buffer_time: std::time::Instant,
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
            PulseWave {
                in_samples: rx,
                phase_inc: 440.0 / spec.freq as f32,
                phase: 0.0,
                volume: 0.1,
                pace: 1,
                step: 4,
                last_buffer_time: std::time::Instant::now(),
            }
        }).unwrap();

        let filename = std::path::Path::new("CH3_out_pkmred.txt");
        if filename.exists() {
            fs::remove_file(filename);
        }
        let mut file = OpenOptions::new().create_new(true).write(true).append(true).open(filename).unwrap();

        APU {
            audio_subsystem,
            latched_div: None,
            div_apu: 0,
            device,
            latched_pace: None,
            ch3: Channel::new(),
            ch3_sample_index: 0,
            period_step: 1023,
            latched_ch3_period: 1024.0,
            clock: 0,
            out_samples: tx,
            buffer: [0; AUDIO_BUFFER_NUM_SAMPLES],
            buffer_i: 0,
            debug_file: file,
            resample_frac: 0.0,
            last_buffer_time: std::time::Instant::now(),
        }
    }

    fn single_tick(&mut self, memory: &mut AddressSpace) {
        if self.div_apu & 2 == 0 {
            if self.ch3.on && self.ch3.length_enabled {
                self.ch3.length_i += 1;
                println!("Length step: {}", self.ch3.length_i);
                if self.ch3.length_i == self.ch3.length {
                    self.ch3.length_enabled = false;
                    self.ch3.on = false;
                    set_ch3_on(self.ch3.on, memory);
                    println!("Length expired!");
                }
            }
            
        // } else if self.div_apu & 4 == 0 {
        //     unimplemented!("CH1 freq sweep!");
        // } else if self.div_apu & 8 == 0 {
        //     unimplemented!("Envelope sweep!");
        }
    }

    fn get_next_sample(&mut self, memory: &AddressSpace) -> Option<u8> {
        self.period_step += 1;
        if self.period_step == self.latched_ch3_period as u32 {
            self.ch3_sample_index = (self.ch3_sample_index + 1) % 32;
            self.latched_ch3_period = 2048.0 - ((ch3_initial_period_high(memory) as f32) * 256.0 + ch3_initial_period_low(memory) as f32);
            self.period_step = 0;
        } else {
            return None;
        }
        let wave_value = ch3_wave_sample(self.ch3_sample_index, memory);
        Some(match ch3_output_level(memory) {
            OutputLevel::Full => wave_value,
            OutputLevel::Half => wave_value >> 1,
            OutputLevel::Quarter => wave_value >> 2,
            OutputLevel::Mute => 0,
        })
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
        let period;

        if self.ch3.on {
            // println!("ON");
            if ch3_length_enable(memory) && !self.ch3.length_enabled {
                self.ch3.length_enabled = true;
                self.ch3.length = ch3_initial_len(memory);
                self.ch3.length_i = 0;
                println!("Length enabled!");
            }
            
            let outcome = self.get_next_sample(memory);
            if outcome.is_none() {
                // wave_value = 0;
                return
            } else {
                wave_value = outcome.unwrap();
            }
            period = self.latched_ch3_period;
        } else {
            wave_value = 255;
            period = 1.0;
        }

        // Fill buffer with raw sample rate, when enough samples collected resample to 44100
        let mut sampling_ratio = TARGET_SAMPLE_RATE as f32 / (2097152.0 / period as f32);
        self.resample_frac += sampling_ratio.fract();
        if self.resample_frac >= 1.0 {
            self.resample_frac -= 1.0;
            sampling_ratio += 1.0;
        }
        let num_rep_samples = sampling_ratio.floor() as u32;

        for _ in 0..num_rep_samples {
            self.buffer[self.buffer_i] = wave_value;
            // let f_value = (wave_value as f32 / -7.5) + 1.0; 
            // let f_value: f32 = if ch3_on(memory) {1.0} else {0.0}; 
            // write!(self.debug_file, "{}", f_value);
            // self.debug_file.write_all(&f_value.to_be_bytes());
            self.buffer_i += 1;
            if self.buffer_i == self.buffer.len() {
                self.buffer_i = 0;
                let mut analog_buffer = [0f32; 2*AUDIO_BUFFER_NUM_SAMPLES];
                for (i, x) in self.buffer.iter().enumerate() {
                    let wave_analog;
                    if *x == 255{
                        wave_analog = 0.0;
                    } else {
                        wave_analog = (*x as f32 / -7.5) + 1.0;
                    }
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
        let device_status = self.device.status();
        if !apu_enabled(memory) {
            if device_status == AudioStatus::Playing {
                // Pause playback
                self.device.pause();
                // println!("Playback paused");
            }
            return;
        } else {
            if apu_enabled(memory) && device_status != AudioStatus::Playing {
                // Start playback
                self.device.resume();
                // println!("Playback enabled");
            }
        }
        let div = (memory.read(DIV_ADDR) >> 4) & 1;
        if div == 1 && self.latched_div.unwrap_or(0) == 1 {
            self.div_apu += 1; // ticks at 512 Hz
            self.single_tick(memory);
        }

        for i in 0..nticks {
            self.clock += 1;
            if self.clock % 2 == 0{
                self.ch3_tick(memory);
            }
        }

        self.latched_div = Some(div & 1);
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
    if (memory.read(NR10_ADDR) >> 3) & 1 == 0{
        SweepDirection::Inc
    } else {
        SweepDirection::Dec
    }
}

pub fn ch1_period_sweep_step(memory: &AddressSpace) -> u8 {
    return memory.read(NR10_ADDR) & 7;
}

pub fn ch1_wave_duty(memory: &AddressSpace) -> WaveDuty {
    match (memory.read(NR11_ADDR) >> 6) & 3 {
        0 => WaveDuty::Eight,
        1 => WaveDuty::Quarter,
        2 => WaveDuty::Half,
        3 => WaveDuty::Quarter,
        x => unreachable!("Invalid value for ch1_wave_duty: {x}"),
    }
}

pub fn ch1_initial_len(memory: &AddressSpace) -> u8 {
    return memory.apu_read(NR11_ADDR) & 0x3F;
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
    return (memory.read(NR12_ADDR) >> 3) != 0;
}

pub fn ch1_initial_period_low(memory: &AddressSpace) -> u8 {
    return memory.apu_read(NR13_ADDR);
}

pub fn ch1_initial_period_high(memory: &AddressSpace) -> u8 {
    return memory.apu_read(NR14_ADDR) & 0x7;
}

pub fn ch1_length_enable(memory: &AddressSpace) -> bool {
    return (memory.read(NR14_ADDR) >> 6) & 1 == 1;
}

pub fn ch1_trigger(memory: &AddressSpace) -> bool {
    return (memory.apu_read(NR14_ADDR) >> 7) & 1 == 1;
}

pub fn ch2_wave_duty(memory: &AddressSpace) -> WaveDuty {
    match (memory.read(NR21_ADDR) >> 6) & 3 {
        0 => WaveDuty::Eight,
        1 => WaveDuty::Quarter,
        2 => WaveDuty::Half,
        3 => WaveDuty::Quarter,
        x => unreachable!("Invalid value for ch1_wave_duty: {x}"),
    }
}

pub fn ch2_initial_len(memory: &AddressSpace) -> u8 {
    return memory.apu_read(NR21_ADDR) & 0x3F;
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
    return (memory.read(NR22_ADDR) >> 3) != 0;
}

pub fn ch2_initial_period_low(memory: &AddressSpace) -> u8 {
    return memory.apu_read(NR23_ADDR);
}

pub fn ch2_initial_period_high(memory: &AddressSpace) -> u8 {
    return memory.apu_read(NR24_ADDR) & 0x7;
}

pub fn ch2_length_enable(memory: &AddressSpace) -> bool {
    return (memory.read(NR24_ADDR) >> 6) & 1 == 1;
}

pub fn ch2_trigger(memory: &AddressSpace) -> bool {
    return (memory.apu_read(NR24_ADDR) >> 7) & 1 == 1;
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
    return (memory.read(NR42_ADDR) >> 3) != 0;
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