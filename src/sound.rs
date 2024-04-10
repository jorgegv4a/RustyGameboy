extern crate sdl2;

use sdl2::AudioSubsystem;
use sdl2::audio::{AudioCallback, AudioDevice, AudioSpecDesired};

use crate::constants::*;
use crate::memory::AddressSpace;


struct SquareWave {
    phase_inc: f32,
    phase: f32,
    volume: f32
}

impl AudioCallback for SquareWave {
    type Channel = f32;

    fn callback(&mut self, out: &mut [f32]) {
        println!("Called! out len : {}, out[0]: {}", out.len(), out[0]);
        // Generate a square wave
        for x in out.iter_mut() {
            // *x = if self.phase <= 0.5 {
            //     self.volume
            // } else {
            //     -self.volume
            // };
            *x = self.phase - 0.5;
            self.phase = (self.phase + self.phase_inc) % 1.0;
        }
    }
}

struct PulseWave {
    phase_inc: f32,
    phase: f32,
    volume: f32,
    pace: u8,
    step: u8,
}

impl AudioCallback for PulseWave {
    type Channel = f32;

    fn callback(&mut self, out: &mut [f32]) {
        println!("Called! out len : {}, out[0]: {}", out.len(), out[0]);
        // Generate a square wave
        let mut prev = self.pace as f32;
        for (i, x) in out.iter_mut().enumerate() {
            // *x = if self.phase <= 0.5 {
            //     self.volume
            // } else {
            //     -self.volume
            // };

            // *x = self.phase - 0.5;
            // self.phase = (self.phase + self.phase_inc) % 1.0;

            *x = prev as f32 + prev as f32 / (2f32.powf(self.step as f32));
            if i % 2 == 0 {
                *x = *x * -1.0;
            }
            println!("prev now {prev}");
            prev = *x;
        }
    }
}


pub struct Channel {
    on: bool,
    volume: u8,
    length: u8,
    freq: u8
}

impl Channel {
    pub fn new() -> Channel {
        Channel {
            on: false,
            volume: 0,
            length: 255,
            freq: 0,
        }
    }
}

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


fn apu_enabled(memory: &AddressSpace) -> bool {
    return (memory.read(NR52_ADDR) >> APU_ENABLE_BIT) & 1 == 1;
}


fn ch1_on(memory: &AddressSpace) -> bool {
    return (memory.read(NR52_ADDR) >> APU_CH1_ON_BIT) & 1 == 1;
}

fn ch2_on(memory: &AddressSpace) -> bool {
    return (memory.read(NR52_ADDR) >> APU_CH2_ON_BIT) & 1 == 1;
}

fn ch3_on(memory: &AddressSpace) -> bool {
    return (memory.read(NR52_ADDR) >> APU_CH3_ON_BIT) & 1 == 1;
}

fn ch4_on(memory: &AddressSpace) -> bool {
    return (memory.read(NR52_ADDR) >> APU_CH4_ON_BIT) & 1 == 1;
}

fn master_left_volume(memory: &AddressSpace) -> u8 {
    return (memory.read(NR50_ADDR) >> 4) & 7;
}

fn master_right_volume(memory: &AddressSpace) -> u8 {
    return memory.read(NR50_ADDR) & 7;
}

fn ch1_pan_left(memory: &AddressSpace) -> bool {
    return (memory.read(NR51_ADDR) >> APU_CH1_PAN_LEFT_BIT) & 1 == 1;
}

fn ch2_pan_left(memory: &AddressSpace) -> bool {
    return (memory.read(NR51_ADDR) >> APU_CH2_PAN_LEFT_BIT) & 1 == 1;
}

fn ch3_pan_left(memory: &AddressSpace) -> bool {
    return (memory.read(NR51_ADDR) >> APU_CH3_PAN_LEFT_BIT) & 1 == 1;
}

fn ch4_pan_left(memory: &AddressSpace) -> bool {
    return (memory.read(NR51_ADDR) >> APU_CH4_PAN_LEFT_BIT) & 1 == 1;
}

fn ch1_pan_right(memory: &AddressSpace) -> bool {
    return (memory.read(NR51_ADDR) >> APU_CH1_PAN_RIGHT_BIT) & 1 == 1;
}

fn ch2_pan_right(memory: &AddressSpace) -> bool {
    return (memory.read(NR51_ADDR) >> APU_CH2_PAN_RIGHT_BIT) & 1 == 1;
}

fn ch3_pan_right(memory: &AddressSpace) -> bool {
    return (memory.read(NR51_ADDR) >> APU_CH3_PAN_RIGHT_BIT) & 1 == 1;
}

fn ch4_pan_right(memory: &AddressSpace) -> bool {
    return (memory.read(NR51_ADDR) >> APU_CH4_PAN_RIGHT_BIT) & 1 == 1;
}

fn ch1_period_sweep_pace(memory: &AddressSpace) -> u8 {
    return (memory.read(NR10_ADDR) >> 4) & 7;
}

fn ch1_sweep_direction(memory: &AddressSpace) -> SweepDirection {
    if (memory.read(NR10_ADDR) >> 3) & 1 == 0{
        SweepDirection::Inc
    } else {
        SweepDirection::Dec
    }
}

fn ch1_period_sweep_step(memory: &AddressSpace) -> u8 {
    return memory.read(NR10_ADDR) & 7;
}

fn ch1_wave_duty(memory: &AddressSpace) -> WaveDuty {
    match (memory.read(NR11_ADDR) >> 6) & 3 {
        0 => WaveDuty::Eight,
        1 => WaveDuty::Quarter,
        2 => WaveDuty::Half,
        3 => WaveDuty::Quarter,
        x => unreachable!("Invalid value for ch1_wave_duty: {x}"),
    }
}

fn ch1_initial_len(memory: &AddressSpace) -> u8 {
    return memory.apu_read(NR11_ADDR) & 0x3F;
}

fn ch1_initial_volume(memory: &AddressSpace) -> u8 {
    return (memory.read(NR12_ADDR) >> 4) & 0xF;
}

fn ch1_envelope_direction(memory: &AddressSpace) -> EnvelopeDirection {
    if (memory.read(NR12_ADDR) >> 3) & 1 == 0 {
        EnvelopeDirection::Dec
    } else {
        EnvelopeDirection::Inc
    }
}

fn ch1_envelope_sweep_pace(memory: &AddressSpace) -> u8 {
    return memory.read(NR12_ADDR) & 0x7;
}

fn ch1_dac_on(memory: &AddressSpace) -> bool {
    return (memory.read(NR12_ADDR) >> 3) != 0;
}

fn ch1_initial_period_low(memory: &AddressSpace) -> u8 {
    return memory.apu_read(NR13_ADDR);
}

fn ch1_initial_period_high(memory: &AddressSpace) -> u8 {
    return memory.apu_read(NR14_ADDR) & 0x7;
}

fn ch1_length_enable(memory: &AddressSpace) -> bool {
    return (memory.read(NR14_ADDR) >> 6) & 1 == 1;
}

fn ch1_trigger(memory: &AddressSpace) -> bool {
    return (memory.apu_read(NR14_ADDR) >> 7) & 1 == 1;
}

fn ch2_wave_duty(memory: &AddressSpace) -> WaveDuty {
    match (memory.read(NR21_ADDR) >> 6) & 3 {
        0 => WaveDuty::Eight,
        1 => WaveDuty::Quarter,
        2 => WaveDuty::Half,
        3 => WaveDuty::Quarter,
        x => unreachable!("Invalid value for ch1_wave_duty: {x}"),
    }
}

fn ch2_initial_len(memory: &AddressSpace) -> u8 {
    return memory.apu_read(NR21_ADDR) & 0x3F;
}

fn ch2_initial_volume(memory: &AddressSpace) -> u8 {
    return (memory.read(NR22_ADDR) >> 4) & 0xF;
}

fn ch2_envelope_direction(memory: &AddressSpace) -> EnvelopeDirection {
    if (memory.read(NR22_ADDR) >> 3) & 1 == 0 {
        EnvelopeDirection::Dec
    } else {
        EnvelopeDirection::Inc
    }
}

fn ch2_envelope_sweep_pace(memory: &AddressSpace) -> u8 {
    return memory.read(NR22_ADDR) & 0x7;
}

fn ch2_dac_on(memory: &AddressSpace) -> bool {
    return (memory.read(NR22_ADDR) >> 3) != 0;
}

fn ch2_initial_period_low(memory: &AddressSpace) -> u8 {
    return memory.apu_read(NR23_ADDR);
}

fn ch2_initial_period_high(memory: &AddressSpace) -> u8 {
    return memory.apu_read(NR24_ADDR) & 0x7;
}

fn ch2_length_enable(memory: &AddressSpace) -> bool {
    return (memory.read(NR24_ADDR) >> 6) & 1 == 1;
}

fn ch2_trigger(memory: &AddressSpace) -> bool {
    return (memory.apu_read(NR24_ADDR) >> 7) & 1 == 1;
}

fn ch3_dac_on(memory: &AddressSpace) -> bool {
    return (memory.read(NR30_ADDR) >> 7) & 1 == 1;
}

fn ch3_initial_len(memory: &AddressSpace) -> u8 {
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

fn ch3_initial_period_low(memory: &AddressSpace) -> u8 {
    return memory.apu_read(NR33_ADDR);
}

fn ch3_initial_period_high(memory: &AddressSpace) -> u8 {
    return memory.apu_read(NR34_ADDR) & 0x7;
}

fn ch3_length_enable(memory: &AddressSpace) -> bool {
    return (memory.read(NR34_ADDR) >> 6) & 1 == 1;
}

fn ch3_trigger(memory: &AddressSpace) -> bool {
    return (memory.apu_read(NR34_ADDR) >> 7) & 1 == 1;
}

fn ch3_wave_sample(memory: &AddressSpace, index: u8) -> u8 {
    if index >= 32 {
        panic!("Invalid ch3 wave sample: {index}");
    }
    let item_byte = memory.read((WAVE_RANGE_START + index as u16/ 2));
    if index % 2 == 1 {
        item_byte & 0xF
    } else {
        (item_byte >> 4) & 0xF
    }
}

fn ch4_initial_len(memory: &AddressSpace) -> u8 {
    return memory.apu_read(NR41_ADDR) & 0x3F;
}

fn ch4_initial_volume(memory: &AddressSpace) -> u8 {
    return (memory.read(NR42_ADDR) >> 4) & 0xF;
}

fn ch4_envelope_direction(memory: &AddressSpace) -> EnvelopeDirection {
    if (memory.read(NR42_ADDR) >> 3) & 1 == 0 {
        EnvelopeDirection::Dec
    } else {
        EnvelopeDirection::Inc
    }
}

fn ch4_envelope_sweep_pace(memory: &AddressSpace) -> u8 {
    return memory.read(NR42_ADDR) & 0x7;
}

fn ch4_dac_on(memory: &AddressSpace) -> bool {
    return (memory.read(NR42_ADDR) >> 3) != 0;
}


pub struct APU {
    audio_subsystem: AudioSubsystem,
    latched_div: Option<u8>,
    div_apu: u64,
    device: AudioDevice<PulseWave>,
    latched_pace: Option<u8>,
}


impl APU {
    pub fn new(audio_subsystem: AudioSubsystem) -> APU {
        let desired_spec = AudioSpecDesired {
            freq: Some(44100),
            channels: Some(1),  // mono
            samples: Some(512),       // default sample size
        };
        
        let device = audio_subsystem.open_playback(None, &desired_spec, |spec| {
                // initialize the audio callback
            // SquareWave {
            PulseWave {
                phase_inc: 440.0 / spec.freq as f32,
                phase: 0.0,
                volume: 0.1,
                pace: 1,
                step: 4,
            }
        }).unwrap();
        
        // Start playback
        // device.resume();

        // std::thread::sleep(std::time::Duration::from_millis(3000));

        APU {
            audio_subsystem,
            latched_div: None,
            div_apu: 0,
            device,
            latched_pace: None,
        }
    }

    fn single_tick(&mut self, memory: &mut AddressSpace) {
        // if self.div_apu & 2 == 0 {
        //     unimplemented!("Sound length!");
        // } else if self.div_apu & 4 == 0 {
        //     unimplemented!("CH1 freq sweep!");
        // } else if self.div_apu & 8 == 0 {
        //     unimplemented!("Envelope sweep!");
        // }
    }

    pub fn tick(&mut self, nticks: u8, memory: &mut AddressSpace) {
        let div = (memory.read(DIV_ADDR) >> 4) & 1;
        if div == 1 && self.latched_div.unwrap_or(0) == 1 {
            // self.device.resume();
            self.div_apu += 1;
            self.single_tick(memory);
        }

        self.latched_div = Some(div & 1);
    }
}