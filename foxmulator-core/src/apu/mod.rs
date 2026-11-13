//! Audio Processing Unit (APU).
//!
//! Implements the four DMG sound channels (two square channels with envelope
//! and sweep, a wave channel and a noise channel), the 512 Hz frame sequencer,
//! NR50/NR51 mixing and 44.1 kHz stereo sample generation.
//!
//! Reference: Pan Docs, "Audio"; FoxMulator JavaScript APU in
//! `FoxMulator_1.0.1_Link_Dual.html`.
//!
//! Limitations (see `docs/audit.md`): some revision-specific analog behavior
//! is not modelled; samples are produced at a fixed 44100 Hz. CGB double speed
//! is scaled by the MMU system clock before this APU is ticked.

pub const NR10: u16 = 0xFF10;
pub const NR11: u16 = 0xFF11;
pub const NR12: u16 = 0xFF12;
pub const NR13: u16 = 0xFF13;
pub const NR14: u16 = 0xFF14;
pub const NR21: u16 = 0xFF16;
pub const NR22: u16 = 0xFF17;
pub const NR23: u16 = 0xFF18;
pub const NR24: u16 = 0xFF19;
pub const NR30: u16 = 0xFF1A;
pub const NR31: u16 = 0xFF1B;
pub const NR32: u16 = 0xFF1C;
pub const NR33: u16 = 0xFF1D;
pub const NR34: u16 = 0xFF1E;
pub const NR41: u16 = 0xFF20;
pub const NR42: u16 = 0xFF21;
pub const NR43: u16 = 0xFF22;
pub const NR44: u16 = 0xFF23;
pub const NR50: u16 = 0xFF24;
pub const NR51: u16 = 0xFF25;
pub const NR52: u16 = 0xFF26;
pub const WAVE_RAM_START: u16 = 0xFF30;
pub const WAVE_RAM_END: u16 = 0xFF3F;

/// CPU clock in Hz (normal speed).
const CPU_CLOCK: u32 = 4_194_304;
/// Frame sequencer step period in T-cycles (512 Hz).
const FRAME_SEQ_PERIOD: u32 = 8192;

const DUTY_TABLE: [[u8; 8]; 4] = [
    [0, 0, 0, 0, 0, 0, 0, 1],
    [1, 0, 0, 0, 0, 0, 0, 1],
    [1, 0, 0, 0, 0, 1, 1, 1],
    [0, 1, 1, 1, 1, 1, 1, 0],
];

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
struct SquareChannel {
    enabled: bool,
    dac: bool,
    duty: u8,
    duty_step: u8,
    length_counter: u16,
    length_enable: bool,
    vol: u8,
    vol_init: u8,
    env_period: u8,
    env_timer: u8,
    env_dir_up: bool,
    env_active: bool,
    freq: u16,
    timer: i32,

    // Sweep (channel 1 only).
    sweep_period: u8,
    sweep_shift: u8,
    sweep_neg: bool,
    sweep_timer: u8,
    sweep_shadow: u16,
    sweep_enabled: bool,
    sweep_neg_used: bool,
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
struct WaveChannel {
    enabled: bool,
    dac: bool,
    length_counter: u16,
    length_enable: bool,
    vol_shift: u8,
    freq: u16,
    timer: i32,
    position: u8,
    sample_latch: u8,
    wave_ram: [u8; 16],
    /// DMG exposes wave RAM only during the narrow window in which CH3 has
    /// just fetched a byte.  CGB does not use this arbitration rule.
    wave_form_just_read: bool,
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
struct NoiseChannel {
    enabled: bool,
    dac: bool,
    length_counter: u16,
    length_enable: bool,
    vol: u8,
    vol_init: u8,
    env_period: u8,
    env_timer: u8,
    env_dir_up: bool,
    env_active: bool,
    clock_shift: u8,
    divisor: u8,
    lfsr_width: bool,
    lfsr: u16,
    timer: i32,
}

impl SquareChannel {
    /// Advance the duty step timer by `step` T-cycles.
    fn tick(&mut self, step: u32) {
        if !self.enabled {
            return;
        }
        self.timer -= step as i32;
        while self.timer <= 0 {
            self.timer += (2048 - self.freq as i32) * 4;
            self.duty_step = (self.duty_step + 1) & 7;
        }
    }
}

impl WaveChannel {
    /// Advance the wave sample position by `step` T-cycles.
    fn tick(&mut self, step: u32) {
        self.wave_form_just_read = false;
        if !self.enabled {
            return;
        }
        self.timer -= step as i32;
        while self.timer <= 0 {
            self.timer += (2048 - self.freq as i32) * 2;
            self.position = (self.position + 1) & 31;
            let byte = self.wave_ram[(self.position >> 1) as usize & 15];
            self.sample_latch = if (self.position & 1) != 0 {
                byte & 0x0F
            } else {
                byte >> 4
            };
            self.wave_form_just_read = true;
        }
        // A CPU access is valid only when this tick ended on the fetch
        // boundary.  If additional cycles elapsed after the fetch, the
        // hardware bus is no longer available to the CPU.
        // The CPU-visible bus remains available for roughly two clocks after
        // the fetch.  APU::tick advances in small instruction-sized chunks,
        // so retain a four-T-cycle window to cover the boundary without
        // exposing the backing RAM for the rest of the period.
        if self.timer < (2048 - self.freq as i32) * 2 - 4 {
            self.wave_form_just_read = false;
        }
    }
}

impl NoiseChannel {
    /// Advance the LFSR by `step` T-cycles.
    fn tick(&mut self, step: u32) {
        if !self.enabled {
            return;
        }
        self.timer -= step as i32;
        while self.timer <= 0 {
            let divisor = [8i32, 16, 32, 48, 64, 80, 96, 112][self.divisor as usize & 7];
            self.timer += divisor << self.clock_shift;
            let xor = (self.lfsr & 1) ^ ((self.lfsr >> 1) & 1);
            self.lfsr >>= 1;
            self.lfsr |= xor << 14;
            if self.lfsr_width {
                self.lfsr = (self.lfsr & !0x40) | (xor << 6);
            }
        }
    }
}

/// APU state and audio sample queue.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Apu {
    pub enabled: bool,
    /// DMG and CGB differ when NR52 powers the APU off: DMG preserves the
    /// length counters, while CGB clears them.
    cgb: bool,
    nr50: u8,
    nr51: u8,
    frame_seq_step: u8,
    frame_seq_counter: u32,
    skip_frame: bool,
    ch1: SquareChannel,
    ch2: SquareChannel,
    ch3: WaveChannel,
    ch4: NoiseChannel,

    sample_rate: u32,
    cycles_per_sample: f32,
    sample_accum: f32,
    /// Interleaved stereo f32 samples (left, right).
    output: Vec<f32>,
    hp_prev_in: [f32; 2],
    hp_prev_out: [f32; 2],
    /// Master volume multiplier applied to every produced sample.
    pub volume: f32,
}

impl Default for Apu {
    fn default() -> Self {
        Self::new()
    }
}

impl Apu {
    pub fn new() -> Self {
        Self {
            enabled: true,
            cgb: false,
            nr50: 0x77,
            nr51: 0xF3,
            frame_seq_step: 0,
            frame_seq_counter: 0,
            skip_frame: false,
            ch1: SquareChannel::default(),
            ch2: SquareChannel::default(),
            ch3: WaveChannel::default(),
            ch4: NoiseChannel::default(),
            sample_rate: 44_100,
            cycles_per_sample: CPU_CLOCK as f32 / 44_100.0,
            sample_accum: 0.0,
            output: Vec::new(),
            hp_prev_in: [0.0; 2],
            hp_prev_out: [0.0; 2],
            volume: 1.0,
        }
    }

    /// Select the hardware model used for model-dependent APU behavior.
    pub fn set_cgb(&mut self, cgb: bool) {
        self.cgb = cgb;
    }

    pub fn set_skip_frame_after_power_on(&mut self, divider_phase: u32) {
        self.skip_frame = divider_phase >= FRAME_SEQ_PERIOD / 2;
    }

    fn frame_events(&mut self, count: u32) {
        if !self.enabled {
            return;
        }
        for _ in 0..count {
            if self.skip_frame {
                self.skip_frame = false;
            } else {
                self.frame_sequencer_tick();
            }
        }
    }

    pub fn timer_frame_event(&mut self) {
        self.frame_events(1);
    }

    /// Read an APU register (NR10-NR52 or wave RAM).
    pub fn read_register(&self, addr: u16) -> u8 {
        match addr {
            NR10 => {
                (self.ch1.sweep_period << 4)
                    | ((self.ch1.sweep_neg as u8) << 3)
                    | self.ch1.sweep_shift
                    | 0x80
            }
            NR11 => (self.ch1.duty << 6) | 0x3F,
            NR12 => {
                (self.ch1.vol_init << 4) | ((self.ch1.env_dir_up as u8) << 3) | self.ch1.env_period
            }
            NR13 => 0xFF,
            NR14 => ((self.ch1.length_enable as u8) << 6) | 0xBF,
            NR21 => (self.ch2.duty << 6) | 0x3F,
            NR22 => {
                (self.ch2.vol_init << 4) | ((self.ch2.env_dir_up as u8) << 3) | self.ch2.env_period
            }
            NR23 => 0xFF,
            NR24 => ((self.ch2.length_enable as u8) << 6) | 0xBF,
            NR30 => ((self.ch3.dac as u8) << 7) | 0x7F,
            NR31 => 0xFF,
            NR32 => (self.ch3.vol_shift << 5) | 0x9F,
            NR33 => 0xFF,
            NR34 => ((self.ch3.length_enable as u8) << 6) | 0xBF,
            NR41 => 0xFF,
            NR42 => {
                (self.ch4.vol_init << 4) | ((self.ch4.env_dir_up as u8) << 3) | self.ch4.env_period
            }
            NR43 => {
                (self.ch4.clock_shift << 4) | ((self.ch4.lfsr_width as u8) << 3) | self.ch4.divisor
            }
            NR44 => ((self.ch4.length_enable as u8) << 6) | 0xBF,
            NR50 => self.nr50,
            NR51 => self.nr51,
            NR52 => {
                let mut v = if self.enabled { 0x80 } else { 0 };
                if self.ch1.enabled {
                    v |= 0x01;
                }
                if self.ch2.enabled {
                    v |= 0x02;
                }
                if self.ch3.enabled {
                    v |= 0x04;
                }
                if self.ch4.enabled {
                    v |= 0x08;
                }
                v | 0x70
            }
            WAVE_RAM_START..=WAVE_RAM_END => {
                if self.ch3.enabled {
                    if self.cgb || self.ch3.wave_form_just_read {
                        self.ch3.wave_ram[(self.ch3.position >> 1) as usize & 15]
                    } else {
                        0xFF
                    }
                } else {
                    self.ch3.wave_ram[(addr - WAVE_RAM_START) as usize]
                }
            }
            _ => 0xFF,
        }
    }

    /// Write an APU register (NR10-NR52 or wave RAM).
    pub fn write_register(&mut self, addr: u16, val: u8) {
        if addr == NR52 {
            if (val & 0x80) == 0 {
                if self.enabled {
                    self.power_off();
                }
            } else if !self.enabled {
                self.enabled = true;
                self.frame_seq_step = 0;
                self.frame_seq_counter = 0;
            }
            return;
        }

        // Wave RAM remains writable while the APU is off.
        if (WAVE_RAM_START..=WAVE_RAM_END).contains(&addr) {
            if self.ch3.enabled && !self.cgb {
                if self.ch3.wave_form_just_read {
                    let index = (self.ch3.position >> 1) as usize & 15;
                    self.ch3.wave_ram[index] = val;
                }
            } else if self.ch3.enabled && self.cgb {
                // CGB also routes an active CH3 access through the currently
                // selected wave byte, but does not have the DMG access gate.
                let index = (self.ch3.position >> 1) as usize & 15;
                self.ch3.wave_ram[index] = val;
            } else {
                self.ch3.wave_ram[(addr - WAVE_RAM_START) as usize] = val;
            }
            return;
        }

        if !self.enabled {
            // DMG hardware still accepts length-counter loads while NR52 is
            // off. CGB clears/ignores these channel registers until power is
            // restored. Other register writes remain ignored in both models.
            if !self.cgb {
                match addr {
                    NR11 => self.ch1.length_counter = 64 - (val & 0x3F) as u16,
                    NR21 => self.ch2.length_counter = 64 - (val & 0x3F) as u16,
                    NR31 => self.ch3.length_counter = 256 - val as u16,
                    NR41 => self.ch4.length_counter = 64 - (val & 0x3F) as u16,
                    _ => {}
                }
            }
            return;
        }

        match addr {
            NR10 => {
                if self.ch1.sweep_neg && (val & 0x08) == 0 && self.ch1.sweep_neg_used {
                    self.ch1.enabled = false;
                }
                self.ch1.sweep_period = (val >> 4) & 0x07;
                self.ch1.sweep_neg = (val & 0x08) != 0;
                self.ch1.sweep_shift = val & 0x07;
            }
            NR11 => {
                self.ch1.duty = (val >> 6) & 0x03;
                self.ch1.length_counter = 64 - (val & 0x3F) as u16;
            }
            NR12 => {
                self.ch1.vol_init = (val >> 4) & 0x0F;
                self.ch1.env_dir_up = (val & 0x08) != 0;
                self.ch1.env_period = val & 0x07;
                self.ch1.dac = (val & 0xF8) != 0;
                if !self.ch1.dac {
                    self.ch1.enabled = false;
                }
            }
            NR13 => {
                self.ch1.freq = (self.ch1.freq & 0x700) | val as u16;
            }
            NR14 => {
                self.ch1.freq = (self.ch1.freq & 0x0FF) | (((val & 0x07) as u16) << 8);
                let was_len = self.ch1.length_enable;
                self.ch1.length_enable = (val & 0x40) != 0;
                if !was_len
                    && self.ch1.length_enable
                    && (self.frame_seq_step & 1) == 1
                    && self.ch1.length_counter > 0
                {
                    self.ch1.length_counter -= 1;
                    if self.ch1.length_counter == 0 {
                        self.ch1.enabled = false;
                    }
                }
                if (val & 0x80) != 0 {
                    self.trigger_ch1();
                }
            }
            NR21 => {
                self.ch2.duty = (val >> 6) & 0x03;
                self.ch2.length_counter = 64 - (val & 0x3F) as u16;
            }
            NR22 => {
                self.ch2.vol_init = (val >> 4) & 0x0F;
                self.ch2.env_dir_up = (val & 0x08) != 0;
                self.ch2.env_period = val & 0x07;
                self.ch2.dac = (val & 0xF8) != 0;
                if !self.ch2.dac {
                    self.ch2.enabled = false;
                }
            }
            NR23 => {
                self.ch2.freq = (self.ch2.freq & 0x700) | val as u16;
            }
            NR24 => {
                self.ch2.freq = (self.ch2.freq & 0x0FF) | (((val & 0x07) as u16) << 8);
                let was_len = self.ch2.length_enable;
                self.ch2.length_enable = (val & 0x40) != 0;
                if !was_len
                    && self.ch2.length_enable
                    && (self.frame_seq_step & 1) == 1
                    && self.ch2.length_counter > 0
                {
                    self.ch2.length_counter -= 1;
                    if self.ch2.length_counter == 0 {
                        self.ch2.enabled = false;
                    }
                }
                if (val & 0x80) != 0 {
                    self.trigger_ch2();
                }
            }
            NR30 => {
                self.ch3.dac = (val & 0x80) != 0;
                if !self.ch3.dac {
                    self.ch3.enabled = false;
                }
            }
            NR31 => {
                self.ch3.length_counter = 256 - val as u16;
            }
            NR32 => {
                self.ch3.vol_shift = (val >> 5) & 0x03;
            }
            NR33 => {
                self.ch3.freq = (self.ch3.freq & 0x700) | val as u16;
            }
            NR34 => {
                self.ch3.freq = (self.ch3.freq & 0x0FF) | (((val & 0x07) as u16) << 8);
                let was_len = self.ch3.length_enable;
                self.ch3.length_enable = (val & 0x40) != 0;
                if !was_len
                    && self.ch3.length_enable
                    && (self.frame_seq_step & 1) == 1
                    && self.ch3.length_counter > 0
                {
                    self.ch3.length_counter -= 1;
                    if self.ch3.length_counter == 0 {
                        self.ch3.enabled = false;
                    }
                }
                if (val & 0x80) != 0 {
                    self.trigger_ch3();
                }
            }
            NR41 => {
                self.ch4.length_counter = 64 - (val & 0x3F) as u16;
            }
            NR42 => {
                self.ch4.vol_init = (val >> 4) & 0x0F;
                self.ch4.env_dir_up = (val & 0x08) != 0;
                self.ch4.env_period = val & 0x07;
                self.ch4.dac = (val & 0xF8) != 0;
                if !self.ch4.dac {
                    self.ch4.enabled = false;
                }
            }
            NR43 => {
                self.ch4.clock_shift = (val >> 4) & 0x0F;
                self.ch4.lfsr_width = (val & 0x08) != 0;
                self.ch4.divisor = val & 0x07;
            }
            NR44 => {
                let was_len = self.ch4.length_enable;
                self.ch4.length_enable = (val & 0x40) != 0;
                if !was_len
                    && self.ch4.length_enable
                    && (self.frame_seq_step & 1) == 1
                    && self.ch4.length_counter > 0
                {
                    self.ch4.length_counter -= 1;
                    if self.ch4.length_counter == 0 {
                        self.ch4.enabled = false;
                    }
                }
                if (val & 0x80) != 0 {
                    self.trigger_ch4();
                }
            }
            NR50 => self.nr50 = val,
            NR51 => self.nr51 = val,
            _ => {}
        }
    }

    fn power_off(&mut self) {
        let lengths = if self.cgb {
            None
        } else {
            Some((
                self.ch1.length_counter,
                self.ch2.length_counter,
                self.ch3.length_counter,
                self.ch4.length_counter,
            ))
        };
        self.enabled = false;
        self.frame_seq_step = 0;
        self.frame_seq_counter = 0;
        self.skip_frame = false;
        self.nr50 = 0;
        self.nr51 = 0;
        let wave_ram = self.ch3.wave_ram;
        self.ch1 = SquareChannel::default();
        self.ch2 = SquareChannel::default();
        self.ch3 = WaveChannel {
            wave_ram,
            ..WaveChannel::default()
        };
        self.ch4 = NoiseChannel::default();
        if let Some((ch1, ch2, ch3, ch4)) = lengths {
            self.ch1.length_counter = ch1;
            self.ch2.length_counter = ch2;
            self.ch3.length_counter = ch3;
            self.ch4.length_counter = ch4;
        }
    }

    fn trigger_ch1(&mut self) {
        self.ch1.enabled = self.ch1.dac;
        if self.ch1.length_counter == 0 {
            self.ch1.length_counter = if self.ch1.length_enable && (self.frame_seq_step & 1) == 1 {
                63
            } else {
                64
            };
        }
        self.ch1.timer = (2048 - self.ch1.freq as i32) * 4;
        self.ch1.vol = self.ch1.vol_init;
        self.ch1.env_timer = if self.ch1.env_period != 0 {
            self.ch1.env_period
        } else {
            8
        };
        self.ch1.env_active = true;

        self.ch1.sweep_shadow = self.ch1.freq;
        self.ch1.sweep_timer = if self.ch1.sweep_period != 0 {
            self.ch1.sweep_period
        } else {
            8
        };
        self.ch1.sweep_enabled = self.ch1.sweep_period != 0 || self.ch1.sweep_shift != 0;
        self.ch1.sweep_neg_used = false;
        if self.ch1.sweep_shift > 0 && self.sweep_calc() > 2047 {
            self.ch1.enabled = false;
        }
    }

    fn trigger_ch2(&mut self) {
        self.ch2.enabled = self.ch2.dac;
        if self.ch2.length_counter == 0 {
            self.ch2.length_counter = if self.ch2.length_enable && (self.frame_seq_step & 1) == 1 {
                63
            } else {
                64
            };
        }
        self.ch2.timer = (2048 - self.ch2.freq as i32) * 4;
        self.ch2.vol = self.ch2.vol_init;
        self.ch2.env_timer = if self.ch2.env_period != 0 {
            self.ch2.env_period
        } else {
            8
        };
        self.ch2.env_active = true;
    }

    fn trigger_ch3(&mut self) {
        if self.ch3.enabled && !self.cgb && self.ch3.timer == 2 {
            let offset = ((self.ch3.position + 1) >> 1) as usize & 15;
            if offset < 4 {
                self.ch3.wave_ram[0] = self.ch3.wave_ram[offset];
            } else {
                let base = offset & !3;
                let block = [
                    self.ch3.wave_ram[base],
                    self.ch3.wave_ram[base + 1],
                    self.ch3.wave_ram[base + 2],
                    self.ch3.wave_ram[base + 3],
                ];
                self.ch3.wave_ram[..4].copy_from_slice(&block);
            }
        }
        self.ch3.enabled = self.ch3.dac;
        if self.ch3.length_counter == 0 {
            self.ch3.length_counter = if self.ch3.length_enable && (self.frame_seq_step & 1) == 1 {
                255
            } else {
                256
            };
        }
        // The trigger reload has a small DMG start delay before the first
        // wave-RAM fetch.  It is observable by the wave-RAM bus tests.
        self.ch3.timer = (2048 - self.ch3.freq as i32) * 2 + 6;
        self.ch3.position = 0;
        self.ch3.sample_latch = 0;
        self.ch3.wave_form_just_read = false;
    }

    fn trigger_ch4(&mut self) {
        self.ch4.enabled = self.ch4.dac;
        if self.ch4.length_counter == 0 {
            self.ch4.length_counter = if self.ch4.length_enable && (self.frame_seq_step & 1) == 1 {
                63
            } else {
                64
            };
        }
        self.ch4.lfsr = 0x7FFF;
        let divisor = [8i32, 16, 32, 48, 64, 80, 96, 112][self.ch4.divisor as usize & 7];
        self.ch4.timer = divisor << self.ch4.clock_shift;
        self.ch4.vol = self.ch4.vol_init;
        self.ch4.env_timer = if self.ch4.env_period != 0 {
            self.ch4.env_period
        } else {
            8
        };
        self.ch4.env_active = true;
    }

    fn sweep_calc(&mut self) -> i32 {
        let delta = (self.ch1.sweep_shadow >> self.ch1.sweep_shift) as i32;
        if self.ch1.sweep_neg {
            self.ch1.sweep_neg_used = true;
            self.ch1.sweep_shadow as i32 - delta
        } else {
            self.ch1.sweep_shadow as i32 + delta
        }
    }

    fn sweep_tick(&mut self) {
        if self.ch1.sweep_timer > 0 {
            self.ch1.sweep_timer -= 1;
        }
        if self.ch1.sweep_timer == 0 {
            self.ch1.sweep_timer = if self.ch1.sweep_period != 0 {
                self.ch1.sweep_period
            } else {
                8
            };
            if self.ch1.sweep_enabled && self.ch1.sweep_period != 0 {
                let new_freq = self.sweep_calc();
                if new_freq > 2047 {
                    self.ch1.enabled = false;
                } else if self.ch1.sweep_shift != 0 {
                    self.ch1.sweep_shadow = new_freq as u16;
                    self.ch1.freq = new_freq as u16;
                    if self.sweep_calc() > 2047 {
                        self.ch1.enabled = false;
                    }
                }
            }
        }
    }

    fn length_tick(&mut self) {
        for ch in [
            &mut self.ch1 as &mut dyn LengthChannel,
            &mut self.ch2,
            &mut self.ch3,
            &mut self.ch4,
        ] {
            ch.length_tick();
        }
    }

    fn envelope_tick(&mut self) {
        for ch in [
            &mut self.ch1 as &mut dyn EnvelopeChannel,
            &mut self.ch2,
            &mut self.ch4,
        ] {
            ch.envelope_tick();
        }
    }

    fn frame_sequencer_tick(&mut self) {
        if matches!(self.frame_seq_step, 0 | 2 | 4 | 6) {
            self.length_tick();
        }
        if matches!(self.frame_seq_step, 2 | 6) {
            self.sweep_tick();
        }
        if self.frame_seq_step == 7 {
            self.envelope_tick();
        }
        self.frame_seq_step = (self.frame_seq_step + 1) & 7;
    }

    /// Advance the APU by `cycles` T-cycles.
    pub fn tick(&mut self, cycles: u32) {
        self.tick_internal(cycles, true, 0);
    }

    /// Advance channels and samples using frame-sequencer events supplied by
    /// the shared timer/divider.
    pub fn tick_with_frame_events(&mut self, cycles: u32, frame_events: u32) {
        self.tick_internal(cycles, false, frame_events);
    }

    fn tick_internal(&mut self, cycles: u32, local_frame: bool, frame_events: u32) {
        let mut remaining = cycles;

        while remaining > 0 {
            let mut step = remaining.min(8);
            if self.ch3.enabled {
                step = step.min((self.ch3.timer.max(1)) as u32);
            }
            if self.ch4.enabled {
                step = step.min((self.ch4.timer.max(1)) as u32);
            }
            if step == 0 {
                step = 1;
            }

            self.ch1.tick(step);
            self.ch2.tick(step);
            self.ch3.tick(step);
            self.ch4.tick(step);

            remaining -= step;
            if local_frame {
                self.frame_seq_counter += step;
                while self.frame_seq_counter >= FRAME_SEQ_PERIOD {
                    self.frame_seq_counter -= FRAME_SEQ_PERIOD;
                    self.frame_events(1);
                }
            }

            self.sample_accum += step as f32;
            while self.sample_accum >= self.cycles_per_sample {
                self.sample_accum -= self.cycles_per_sample;
                let sample = self.mix_sample();
                self.push_sample(sample);
            }
        }
        if !local_frame {
            self.frame_events(frame_events);
        }
    }

    fn mix_sample(&mut self) -> [f32; 2] {
        let mut ch1 = 0.0;
        let mut ch2 = 0.0;
        let mut ch3 = 0.0;
        let mut ch4 = 0.0;

        if self.ch1.enabled {
            let duty = DUTY_TABLE[self.ch1.duty as usize & 3][self.ch1.duty_step as usize & 7];
            ch1 = if duty != 0 {
                self.ch1.vol as f32
            } else {
                -(self.ch1.vol as f32)
            };
        }
        if self.ch2.enabled {
            let duty = DUTY_TABLE[self.ch2.duty as usize & 3][self.ch2.duty_step as usize & 7];
            ch2 = if duty != 0 {
                self.ch2.vol as f32
            } else {
                -(self.ch2.vol as f32)
            };
        }
        if self.ch3.enabled {
            let mut sample = self.ch3.sample_latch & 0x0F;
            sample = match self.ch3.vol_shift {
                0 => 0,
                1 => sample,
                2 => sample >> 1,
                _ => sample >> 2,
            };
            ch3 = sample as f32 - 8.0;
        }
        if self.ch4.enabled {
            ch4 = if (self.ch4.lfsr & 1) != 0 {
                -(self.ch4.vol as f32)
            } else {
                self.ch4.vol as f32
            };
        }

        let vol_left = ((self.nr50 >> 4) & 0x07) as f32 + 1.0;
        let vol_right = (self.nr50 & 0x07) as f32 + 1.0;

        let mut left = 0.0;
        let mut right = 0.0;
        if (self.nr51 & 0x10) != 0 {
            left += ch1;
        }
        if (self.nr51 & 0x20) != 0 {
            left += ch2;
        }
        if (self.nr51 & 0x40) != 0 {
            left += ch3;
        }
        if (self.nr51 & 0x80) != 0 {
            left += ch4;
        }
        if (self.nr51 & 0x01) != 0 {
            right += ch1;
        }
        if (self.nr51 & 0x02) != 0 {
            right += ch2;
        }
        if (self.nr51 & 0x04) != 0 {
            right += ch3;
        }
        if (self.nr51 & 0x08) != 0 {
            right += ch4;
        }

        let norm = 4.0 * 15.0 * 8.0;
        let mut left = (left * vol_left) / norm;
        let mut right = (right * vol_right) / norm;

        // Simple DC-blocking high-pass filter.
        let coeff = 0.997;
        let out_left = left - self.hp_prev_in[0] + coeff * self.hp_prev_out[0];
        let out_right = right - self.hp_prev_in[1] + coeff * self.hp_prev_out[1];
        self.hp_prev_in = [left, right];
        self.hp_prev_out = [out_left, out_right];
        left = out_left;
        right = out_right;

        [left.clamp(-1.0, 1.0), right.clamp(-1.0, 1.0)]
    }

    fn push_sample(&mut self, sample: [f32; 2]) {
        // Bound the queue to one second of stereo audio so a consumer that
        // stops draining cannot grow memory without limit.
        const MAX_SAMPLES: usize = 44_100 * 2;
        if self.output.len() >= MAX_SAMPLES {
            self.output.drain(0..2);
        }
        self.output.push(sample[0] * self.volume);
        self.output.push(sample[1] * self.volume);
    }

    /// Drain the queued interleaved stereo samples.
    pub fn take_samples(&mut self) -> Vec<f32> {
        std::mem::take(&mut self.output)
    }

    /// Compatibility helper: drain the queue and mix down to mono i16.
    pub fn generate_samples(&mut self) -> Vec<i16> {
        let stereo = self.take_samples();
        stereo
            .chunks(2)
            .filter_map(|pair| {
                if pair.len() != 2 {
                    return None;
                }
                let mono = (pair[0] + pair[1]) * 0.5;
                Some((mono.clamp(-1.0, 1.0) * i16::MAX as f32) as i16)
            })
            .collect()
    }

    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }
}

trait LengthChannel {
    fn length_tick(&mut self);
}

trait EnvelopeChannel {
    fn envelope_tick(&mut self);
}

impl LengthChannel for SquareChannel {
    fn length_tick(&mut self) {
        if self.length_enable && self.length_counter > 0 {
            self.length_counter -= 1;
            if self.length_counter == 0 {
                self.enabled = false;
            }
        }
    }
}

impl LengthChannel for WaveChannel {
    fn length_tick(&mut self) {
        if self.length_enable && self.length_counter > 0 {
            self.length_counter -= 1;
            if self.length_counter == 0 {
                self.enabled = false;
            }
        }
    }
}

impl LengthChannel for NoiseChannel {
    fn length_tick(&mut self) {
        if self.length_enable && self.length_counter > 0 {
            self.length_counter -= 1;
            if self.length_counter == 0 {
                self.enabled = false;
            }
        }
    }
}

impl EnvelopeChannel for SquareChannel {
    fn envelope_tick(&mut self) {
        if !self.env_active || self.env_period == 0 {
            return;
        }
        if self.env_timer > 0 {
            self.env_timer -= 1;
        }
        if self.env_timer == 0 {
            self.env_timer = if self.env_period != 0 {
                self.env_period
            } else {
                8
            };
            if self.env_dir_up {
                if self.vol < 15 {
                    self.vol += 1;
                }
            } else if self.vol > 0 {
                self.vol -= 1;
            }
            if self.vol == 0 || self.vol == 15 {
                self.env_active = false;
            }
        }
    }
}

impl EnvelopeChannel for NoiseChannel {
    fn envelope_tick(&mut self) {
        if !self.env_active || self.env_period == 0 {
            return;
        }
        if self.env_timer > 0 {
            self.env_timer -= 1;
        }
        if self.env_timer == 0 {
            self.env_timer = if self.env_period != 0 {
                self.env_period
            } else {
                8
            };
            if self.env_dir_up {
                if self.vol < 15 {
                    self.vol += 1;
                }
            } else if self.vol > 0 {
                self.vol -= 1;
            }
            if self.vol == 0 || self.vol == 15 {
                self.env_active = false;
            }
        }
    }
}
