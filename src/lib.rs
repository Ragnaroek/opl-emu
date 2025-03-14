#![cfg_attr(
    not(any(feature = "sdl", feature = "catalog", feature = "web")),
    no_std
)]

#[cfg(test)]
#[path = "./lib_test.rs"]
mod lib_test;

#[cfg(feature = "catalog")]
pub mod catalog;

// SDL

#[cfg(feature = "sdl")]
pub mod backend_sdl;

#[cfg(feature = "sdl")]
pub use backend_sdl::OPL;

#[cfg(feature = "sdl")]
pub fn new() -> Result<backend_sdl::OPL, &'static str> {
    return backend_sdl::new();
}

// Web
#[cfg(feature = "web")]
pub mod backend_web;

#[cfg(feature = "web")]
pub use backend_web::OPL;

#[cfg(feature = "web")]
pub fn new() -> Result<backend_web::OPL, &'static str> {
    return backend_web::new();
}

use core::array::from_fn;
use std::f64::consts::PI;

const OPL_RATE: f64 = 14318180.0 / 288.0;

const TREMOLO_TABLE_SIZE: usize = 52;

const NUM_CHANNELS: usize = 18;

// TODO impl WAVE_PRECISION WAVE_BITS mode
const WAVE_BITS: u32 = 10;
const WAVE_SH: u32 = 32 - WAVE_BITS;

const LFO_SH: u32 = WAVE_SH - 10;
const LFO_MAX: u32 = 256 << LFO_SH;

const ENV_BITS: i32 = 9;
const ENV_MIN: i32 = 0;
const ENV_EXTRA: i32 = ENV_BITS - 9;
const ENV_MAX: i32 = 511 << ENV_EXTRA;
const ENV_LIMIT: i32 = (12 * 256) >> (3 - ENV_EXTRA);

const RATE_SH: u32 = 24;
const RATE_MASK: u32 = (1 << RATE_SH) - 1;

const MUL_SH: i16 = 16;
//how much to substract from the base value for the final attenuation
static KSL_CREATE_TABLE: [u8; 16] = [
    //0 will always be be lower than 7 * 8
    64, 32, 24, 19, 16, 12, 11, 10, 8, 6, 5, 4, 3, 2, 1, 0,
];

static FREQ_CREATE_TABLE: [u8; 16] = [1, 2, 4, 6, 8, 10, 12, 14, 16, 18, 20, 20, 24, 24, 30, 30];
static ATTACK_SAMPLES_TABLE: [u8; 13] = [69, 55, 46, 40, 35, 29, 23, 20, 19, 15, 11, 10, 9];
static ENVELOPE_INCREASE_TABLE: [u8; 13] = [4, 5, 6, 7, 8, 10, 12, 14, 16, 20, 24, 28, 32];

//distance into WaveTable the wave starts
static WAVE_BASE_TABLE: [usize; 8] = [0x000, 0x200, 0x200, 0x800, 0xa00, 0xc00, 0x100, 0x400];

//mask the counter with this
static WAVE_MASK_TABLE: [u16; 8] = [1023, 1023, 511, 511, 1023, 1023, 512, 1023];

//where to start the counter on at keyon
static WAVE_START_TABLE: [u16; 8] = [512, 0, 0, 0, 0, 512, 512, 256];

const SHIFT_KSLBASE: u32 = 16;
const SHIFT_KEYCODE: u32 = 24;

//The lower bits are the shift of the operator vibrato value
//The highest bit is right shifted to generate -1 or 0 for negation
//So taking the highest input value of 7 this gives 3, 7, 3, 0, -3, -7, -3, 0
static VIBRATO_TABLE: [i8; 8] = [1, 0, 1, 30, -127, -128, -127, -98];

static KSL_SHIFT_TABLE: [u8; 4] = [31, 1, 2, 0];

const MASK_KSR: u8 = 0x10;
const MASK_SUSTAIN: u8 = 0x20;
const MASK_VIBRATO: u8 = 0x40;

const AL_CHAR: u32 = 0x20;
const AL_SCALE: u32 = 0x40;
const AL_ATTACK: u32 = 0x60;
const AL_SUS: u32 = 0x80;
const AL_WAVE: u32 = 0xe0;
const AL_FEED_CON: u32 = 0xc0;
const AL_FREQ_L: u32 = 0xa0;
const AL_FREQ_H: u32 = 0xb0;

static VOLUME_HANDLER_TABLE: [VolumeHandler; 5] = [
    template_volume_off,
    template_volume_release,
    template_volume_sustain,
    template_volume_decay,
    template_volume_attack,
];

#[derive(Clone, Debug)]
pub struct Instrument {
    pub m_char: u8,
    pub c_char: u8,
    pub m_scale: u8,
    pub c_scale: u8,
    pub m_attack: u8,
    pub c_attack: u8,
    pub m_sus: u8,
    pub c_sus: u8,
    pub m_wave: u8,
    pub c_wave: u8,
    pub n_conn: u8,
    pub voice: u8,
    pub mode: u8,
}

#[derive(Clone, Debug)]
pub struct AdlSound {
    pub length: u32,
    pub priority: u16,
    pub instrument: Instrument,
    pub block: u8,
    pub data: Vec<u8>,
    pub terminator: u8,
    pub name: String,
}

// State structs for backend impls.

struct ImfState {
    pub data: Vec<u8>,

    pub hack_ptr: usize,
    pub hack_len: usize,
    pub hack_seq_len: usize,
    pub hack_time: u32,
    pub al_time_count: u32,
    pub sq_active: bool,
}

struct AdlState {
    pub sound: AdlSound,
    pub data_ptr: usize,
    pub sound_time_counter: u32,
    pub al_block: u8,
}

// Ende State structs

pub struct OPLSettings {
    pub mixer_rate: u32,
    pub imf_clock_rate: u32,
    pub adl_clock_rate: u32,
}

pub struct Chip {
    channels: [Channel; NUM_CHANNELS],

    //this is used as the base counter for vibrato and tremolo
    lfo_counter: u32,
    lfo_add: u32,

    reg_104: u8,
    reg_08: u8,
    reg_bd: u8,
    vibrato_index: u8,
    tremolo_index: u8,
    vibrato_sign: i8,
    vibrato_shift: u8,
    tremolo_value: u8,
    vibrato_strength: u8,
    tremolo_strength: u8,

    wave_form_mask: u8,
    opl3_active: u8,

    tables: Tables,
}

pub struct ChipValues {
    wave_form_mask: u8,
    opl3_active: u8,
}

#[repr(u8)]
#[derive(PartialEq, Debug, Copy, Clone)]
enum OperatorState {
    OFF,
    RELEASE,
    SUSTAIN,
    DECAY,
    ATTACK,
}

type VolumeHandler = fn(channel: &mut Operator) -> i32;

pub struct Operator {
    vol_handler: VolumeHandler,

    // TODO #if (DBOPL_WAVE == WAVE_HANDLER) ?
    wave_base: usize,
    wave_mask: u32,
    wave_start: u32,
    wave_index: u32,
    wave_add: u32,
    wave_current: u32,

    chan_data: u32,
    freq_mul: u32,
    vibrato: u32,
    sustain_level: i32,
    total_level: i32,
    current_level: i32,
    volume: i32,

    attack_add: u32,
    decay_add: u32,
    release_add: u32,
    rate_index: u32,

    rate_zero: u8,
    key_on: u8,
    //registers, also used to check for changes
    reg_20: u8,
    reg_40: u8,
    reg_60: u8,
    reg_80: u8,
    reg_e0: u8,
    state: OperatorState,
    tremolo_mask: u8,
    vib_strength: u8,
    ksr: u8,
}

impl Operator {
    pub fn new() -> Operator {
        let mut op = Operator {
            vol_handler: template_volume_off,

            wave_base: 0,
            wave_mask: 0,
            wave_start: 0,
            wave_add: 0,
            wave_index: 0,
            wave_current: 0,

            chan_data: 0,
            freq_mul: 0,
            vibrato: 0,
            sustain_level: ENV_MAX,
            total_level: ENV_MAX,
            current_level: ENV_MAX,
            volume: ENV_MAX,

            attack_add: 0,
            decay_add: 0,
            release_add: 0,
            rate_index: 0,

            rate_zero: 1 << (OperatorState::OFF as u8),
            key_on: 0,
            reg_20: 0,
            reg_40: 0,
            reg_60: 0,
            reg_80: 0,
            reg_e0: 0,
            state: OperatorState::OFF,
            tremolo_mask: 0,
            vib_strength: 0,
            ksr: 0,
        };
        op.set_state(OperatorState::OFF);
        op
    }

    fn key_on(&mut self, mask: u8) {
        if self.key_on == 0 {
            //Restart the frequency generator
            // TODO handle #if( DBOPL_WAVE > WAVE_HANDLER ) else case
            self.wave_index = self.wave_start;
            self.rate_index = 0;
            self.set_state(OperatorState::ATTACK);
        }
        self.key_on |= mask;
    }

    fn key_off(&mut self, mask: u8) {
        self.key_on &= !mask;
        if self.key_on == 0 {
            if self.state != OperatorState::OFF {
                self.set_state(OperatorState::RELEASE);
            }
        };
    }

    fn set_state(&mut self, s: OperatorState) {
        self.state = s;
        self.vol_handler = VOLUME_HANDLER_TABLE[s as usize];
    }
}

#[derive(Debug, PartialEq, PartialOrd)]
enum SynthMode {
    SM2AM,
    SM2FM,
    SM3AM,
    SM3FM,
    SM4Start,
    SM3FMFM,
    SM3AMFM,
    SM3FMAM,
    SM3AMAM,
    SM6Start,
    SM2Percussion,
    SM3Percussion,
}

type SynthHandler =
    fn(chip: &mut Chip, channel_ix: usize, samples: usize, output: &mut [i32]) -> usize;

pub struct Channel {
    operator: [Operator; 2],
    synth_handler: SynthHandler,
    chan_data: u32, //Frequency/octave and derived values
    old: [i32; 2],  //Old data for feedback

    feedback: u8,
    reg_b0: u8,
    reg_c0: u8,

    //this should correspond with reg104, bit 6 indicates a Percussion channel, bit 7 indicates a silent channel
    four_mask: u8,
    mask_left: i8, //sign extended values for both channel's panning
    mask_right: i8,
}

impl Channel {
    pub fn new() -> Channel {
        Channel {
            operator: [Operator::new(), Operator::new()],
            chan_data: 0,
            old: [0, 0],
            feedback: 31,
            reg_b0: 0,
            reg_c0: 0,
            four_mask: 0,
            mask_left: -1,
            mask_right: -1,
            synth_handler: channel_block_template_sm2fm,
        }
    }

    pub fn op(&mut self, ix: usize) -> &mut Operator {
        &mut self.operator[ix]
    }

    fn set_chan_data(&mut self, tables: &Tables, data: u32) {
        let change = self.chan_data ^ data;
        self.chan_data = data;
        self.op(0).chan_data = data;
        self.op(1).chan_data = data;
        //since a frequency update triggered this, always update frequency
        operator_update_frequency(self.op(0));
        operator_update_frequency(self.op(1));

        if (change & (0xff << SHIFT_KSLBASE)) != 0 {
            operator_update_attenuation(self.op(0));
            operator_update_attenuation(self.op(1));
        }
        if (change & (0xff << SHIFT_KEYCODE)) != 0 {
            operator_update_rates(self.op(0), tables);
            operator_update_rates(self.op(1), tables);
        }
    }
}

#[derive(PartialEq, Debug)]
struct OpOffset {
    chan: usize,
    op: usize,
}

struct Tables {
    chan_offset_table: [Option<usize>; 32],
    // Stores None, 0 or 1 for the channel offset to apply
    op_offset_table: [Option<OpOffset>; 64],
    tremolo_table: [u8; TREMOLO_TABLE_SIZE],

    //frequency scales for the different multiplications
    freq_mul: [u32; 16],
    //rates for decay and release for rate of this chip
    linear_rates: [u32; 76],
    //best match attack rates for the rate of this chip
    attack_rates: [u32; 76],

    //Layout of the waveform table in 512 entry intervals
    //With overlapping waves we reduce the table to half it's size
    //
    //	|    |//\\|____|WAV7|//__|/\  |____|/\/\|
    //	|\\//|    |    |WAV7|    |  \/|    |    |
    //	|06  |0126|17  |7   |3   |4   |4 5 |5   |
    //
    //6 is just 0 shifted and masked
    wave_table: [i16; 8 * 512],
    mul_table: [u16; 384],
    ksl_table: [u8; 8 * 16],
}

fn init_tables(scale: f64) -> Tables {
    let mut mul_table = [0; 384];
    for i in 0..384 {
        let s = (i * 8) as f64;
        let p = 2.0_f64.powf(-1.0 + (255.0 - s) * (1.0 / 256.0));
        let val = 0.5 + p * (1 << MUL_SH) as f64;
        mul_table[i] = val as u16;
    }

    let mut wave_table = [0; 8 * 512];
    //sine Wave Base
    for i in 0..512 {
        wave_table[0x0200 + i] = (((i as f64 + 0.5) * (PI / 512.0)).sin() * 4084.0) as i16;
        wave_table[0x0000 + i] = -wave_table[0x0200 + i];
    }
    for i in 0..256 {
        wave_table[0x0700 + i] = (0.5
            + ((2.0f64).powf(-1.0 + (255.0 - i as f64 * 8.0) * (1.0 / 256.0))) * 4085.0)
            as i16;
        wave_table[0x6ff - i] = -wave_table[0x0700 + i];
    }
    //	|    |//\\|____|WAV7|//__|/\  |____|/\/\|
    //	|\\//|    |    |WAV7|    |  \/|    |    |
    //	|06  |0126|27  |7   |3   |4   |4 5 |5   |
    for i in 0..256 {
        //fill silence gaps
        wave_table[0x400 + i] = wave_table[0];
        wave_table[0x500 + i] = wave_table[0];
        wave_table[0x900 + i] = wave_table[0];
        wave_table[0xc00 + i] = wave_table[0];
        wave_table[0xd00 + i] = wave_table[0];
        //replicate sines in other pieces
        wave_table[0x800 + i] = wave_table[0x200 + i];
        //double speed sines
        wave_table[0xa00 + i] = wave_table[0x200 + i * 2];
        wave_table[0xb00 + i] = wave_table[0x000 + i * 2];
        wave_table[0xe00 + i] = wave_table[0x200 + i * 2];
        wave_table[0xf00 + i] = wave_table[0x200 + i * 2];
    }
    // TODO Impl WAVE_PRECISION

    //create the ksl table
    let mut ksl_table = [0; 8 * 16];
    for oct in 0..8 {
        let base = oct * 8;
        for i in 0..16 {
            let mut val = base as i32 - KSL_CREATE_TABLE[i] as i32;
            if val < 0 {
                val = 0;
            }
            //*4 for the final range to match attenuation range
            ksl_table[oct as usize * 16 + i] = (val * 4) as u8;
        }
    }

    let mut freq_mul = [0; 16];
    let freq_scale = (0.5 + scale * (1 << (WAVE_SH - 1 - 10)) as f64) as u32;
    for i in 0..16 {
        freq_mul[i] = freq_scale * FREQ_CREATE_TABLE[i] as u32;
    }

    let mut linear_rates = [0; 76];
    for i in 0..76 {
        let (ix, shift_select) = envelope_select(i);
        let shift = RATE_SH + ENV_EXTRA as u32 - shift_select as u32 - 3;
        linear_rates[i as usize] =
            (scale * ((ENVELOPE_INCREASE_TABLE[ix as usize] as u32) << shift) as f64) as u32;
    }

    //generate the best matching attack rate
    let mut attack_rates = [0; 76];
    for i in 0..62 {
        let (ix, shift_select) = envelope_select(i);
        //original amount of samples the attack would take
        let original =
            (((ATTACK_SAMPLES_TABLE[ix as usize] as u32) << shift_select) as f64 / scale) as u32;

        let mut guess_add = (scale
            * ((ENVELOPE_INCREASE_TABLE[ix as usize] as u32) << (RATE_SH - shift_select as u32 - 3))
                as f64) as u32;
        let mut best_add = guess_add;
        let mut best_diff = 1 << 30;

        for _ in 0..16 {
            let mut volume = ENV_MAX;
            let mut samples = 0;
            let mut count = 0;

            while volume > 0 && samples < original * 2 {
                count += guess_add;
                let change = (count >> RATE_SH) as i32;
                count &= RATE_MASK;
                if change != 0 {
                    volume += (!volume * change) >> 3;
                }
                samples += 1;
            }
            let diff = original as i32 - samples as i32;
            let l_diff = diff.abs() as u32;
            //init last on first pass
            if l_diff < best_diff {
                best_diff = l_diff;
                best_add = guess_add;
                if best_diff == 0 {
                    break;
                }
            }
            //below our target
            if diff < 0 {
                //better than the last time
                let mul = ((original as i32 - diff) << 12) / original as i32;
                guess_add = ((guess_add as u64 * mul as u64) >> 12) as u32;
                guess_add += 1;
            } else if diff > 0 {
                let mul = ((original as i32 - diff) << 12) / original as i32;
                guess_add = ((guess_add as u64 * mul as u64) >> 12) as u32;
                guess_add -= 1;
            }
        }
        attack_rates[i as usize] = best_add;
    }
    for i in 62..76 {
        attack_rates[i] = 8 << RATE_SH;
    }

    let mut chan_offset_table = [None; 32];
    for i in 0..32 {
        let mut index = i & 0xf;
        if index >= 9 {
            continue;
        }
        //Make sure the four op channels follow eachother
        if index < 6 {
            index = (index % 3) * 2 + (index / 3);
        }
        if i >= 16 {
            index += 9;
        }
        chan_offset_table[i] = Some(index);
    }

    let op_offset_table = from_fn(|i| {
        if i % 8 >= 6 || ((i / 8) % 4 == 3) {
            return None;
        }
        let mut ch_num = (i / 8) * 3 + (i % 8) % 3;
        if ch_num >= 12 {
            ch_num += 16 - 12;
        }

        let op_num = (i % 8) / 3;

        if let Some(chan_offset) = chan_offset_table[ch_num] {
            Some(OpOffset {
                chan: chan_offset,
                op: op_num,
            })
        } else {
            None
        }
    });

    // create the Tremolo table, just increase and decrease a triangle wave
    let mut tremolo_table = [0; TREMOLO_TABLE_SIZE];
    for i in 0..(TREMOLO_TABLE_SIZE / 2) {
        let val = (i << ENV_EXTRA) as u8;
        tremolo_table[i] = val;
        tremolo_table[TREMOLO_TABLE_SIZE - 1 - i] = val;
    }

    Tables {
        chan_offset_table,
        op_offset_table,
        tremolo_table,
        freq_mul,
        linear_rates,
        attack_rates,
        wave_table,
        ksl_table,
        mul_table,
    }
}

fn envelope_select(val: u8) -> (u8, u8) {
    if val < 13 * 4 {
        // rate 0 - 12
        (val & 3, 12 - (val >> 2))
    } else if val < 15 * 4 {
        // rate 13 - 14
        (val - 12 * 4, 0)
    } else {
        (12, 0)
    }
}

impl Chip {
    // creates a new Chip and set it up to be used.
    pub fn new(rate: u32) -> Chip {
        let channels = from_fn(|_| Channel::new());
        let scale = OPL_RATE / rate as f64;
        Chip {
            channels,
            lfo_counter: 0,
            lfo_add: (0.5 + scale * (1 << LFO_SH) as f64) as u32,
            reg_104: 0,
            reg_08: 0,
            reg_bd: 0,
            vibrato_index: 0,
            tremolo_index: 0,
            vibrato_sign: 0,
            vibrato_shift: 0,
            tremolo_value: 0,
            vibrato_strength: 0,
            tremolo_strength: 0,
            wave_form_mask: 0,
            opl3_active: 0,
            tables: init_tables(scale),
        }
    }

    pub fn setup(&mut self) {
        self.channels[0].four_mask = 0x00 | (1 << 0);
        self.channels[1].four_mask = 0x80 | (1 << 0);
        self.channels[2].four_mask = 0x00 | (1 << 1);
        self.channels[3].four_mask = 0x80 | (1 << 1);
        self.channels[4].four_mask = 0x00 | (1 << 2);
        self.channels[5].four_mask = 0x80 | (1 << 2);

        self.channels[9].four_mask = 0x00 | (1 << 3);
        self.channels[10].four_mask = 0x80 | (1 << 3);
        self.channels[11].four_mask = 0x00 | (1 << 4);
        self.channels[12].four_mask = 0x80 | (1 << 4);
        self.channels[13].four_mask = 0x00 | (1 << 5);
        self.channels[14].four_mask = 0x80 | (1 << 5);

        //mark the percussion channels
        self.channels[6].four_mask = 0x40;
        self.channels[7].four_mask = 0x40;
        self.channels[8].four_mask = 0x40;

        //clear Everything in opl3 mode
        self.write_reg(0x105, 0x1);
        for i in 0..512 {
            if i == 0x105 {
                continue;
            }
            self.write_reg(i, 0xff);
            self.write_reg(i, 0x00);
        }
        self.write_reg(0x105, 0x00);
        //clear everything in opl2 mode
        for i in 0..255 {
            self.write_reg(i, 0xff);
            self.write_reg(i, 0x00);
        }
        self.write_reg(1, 0x20);
    }

    pub fn write_reg(&mut self, reg: u32, val: u8) {
        match reg & 0xf0 {
            0x00 => {
                if reg == 0x01 {
                    self.wave_form_mask = if (val & 0x20) != 0 { 0x7 } else { 0x0 };
                } else if reg == 0x104 {
                    //only detect changes in lowest 6 bits
                    if ((self.reg_104 ^ val) & 0x3f) == 0 {
                        return;
                    }
                    //always keep the highest bit enabled, for checking > 0x80
                    self.reg_104 = 0x80 | (val & 0x3f);
                } else if reg == 0x105 {
                    //MAME says the real opl3 doesn't reset anything on opl3 disable/enable till the next write in another register
                    if ((self.opl3_active ^ val) & 1) == 0 {
                        return;
                    };
                    self.opl3_active = if (val & 1) != 0 { 0xff } else { 0 };
                    //update the 0xc0 register for all channels to signal the switch to mono/stereo handlers
                    for i in 0..NUM_CHANNELS {
                        self.channel_reset_c0(i);
                    }
                } else if reg == 0x08 {
                    self.reg_08 = val;
                }
            }
            0x10 => { /*no-op*/ }
            0x20 | 0x30 => self.regop_write_20(reg, val),
            0x40 | 0x50 => self.regop_write_40(reg, val),
            0x60 | 0x70 => self.regop_write_60(reg, val),
            0x80 | 0x90 => self.regop_write_80(reg, val),
            0xa0 => self.regchan_write_a0(reg, val),
            0xb0 => {
                if reg == 0xbd {
                    self.write_bd(val);
                } else {
                    self.regchan_write_b0(reg, val);
                }
            }
            0xc0 => self.regchan_write_c0(reg, val),
            0xd0 => { /* no-op */ }
            0xe0 | 0xf0 => self.regop_write_e0(reg, val),
            _ => todo!("reg {:x} not implemented", reg & 0xf0),
        }
    }

    fn write_bd(&mut self, val: u8) {
        let change = self.reg_bd ^ val;
        if change == 0 {
            return;
        }
        self.reg_bd = val;

        self.vibrato_strength = if (val & 0x40) != 0 { 0x00 } else { 0x01 };
        self.tremolo_strength = if (val & 0x80) != 0 { 0x00 } else { 0x02 };

        if (val & 0x20) != 0 {
            //drum was just enabled, make sure channel 6 has the right synth
            if (change & 0x20) != 0 {
                if self.opl3_active != 0 {
                    self.channels[6].synth_handler = channel_block_template_sm3percussion;
                } else {
                    self.channels[6].synth_handler = channel_block_template_sm2percussion;
                }
            }
            //Bass Drum
            if (val & 0x10) != 0 {
                self.channels[6].op(0).key_on(0x2);
                self.channels[6].op(1).key_on(0x2);
            } else {
                self.channels[6].op(0).key_off(0x2);
                self.channels[6].op(1).key_off(0x2);
            }
            //Hi-Hat
            if (val & 0x1) != 0 {
                self.channels[7].op(0).key_on(0x2);
            } else {
                self.channels[7].op(0).key_off(0x2);
            }
            //Snare
            if (val & 0x8) != 0 {
                self.channels[7].op(1).key_on(0x2);
            } else {
                self.channels[7].op(1).key_off(0x2);
            }
            //Tom-Tom
            if (val & 0x4) != 0 {
                self.channels[8].op(0).key_on(0x2);
            } else {
                self.channels[8].op(0).key_off(0x2);
            }
            //Top Cymbal
            if (val & 0x2) != 0 {
                self.channels[8].op(1).key_on(0x2);
            } else {
                self.channels[8].op(1).key_off(0x2);
            }
        //toggle keyoffs when we turn off the percussion
        } else if (change & 0x20) != 0 {
            //trigger a reset to setup the original synth handler
            self.channel_reset_c0(6);
            self.channels[6].op(0).key_off(0x2);
            self.channels[6].op(1).key_off(0x2);
            self.channels[7].op(0).key_off(0x2);
            self.channels[7].op(1).key_off(0x2);
            self.channels[8].op(0).key_off(0x2);
            self.channels[8].op(1).key_off(0x2);
        }
    }

    fn regop_write(
        &mut self,
        reg: u32,
        val: u8,
        f: fn(op: &mut Operator, tables: &Tables, chip: &ChipValues, val: u8),
    ) {
        let ix = ((reg >> 3) & 0x20) | (reg & 0x1f);
        if let Some(offset) = &self.tables.op_offset_table[ix as usize] {
            let op = &mut self.channels[offset.chan].operator[offset.op];
            let chip_values = ChipValues {
                wave_form_mask: self.wave_form_mask,
                opl3_active: self.opl3_active,
            };
            f(op, &self.tables, &chip_values, val);
        }
    }

    fn regop_write_20(&mut self, reg: u32, val: u8) {
        self.regop_write(reg, val, operator_write_20);
    }

    fn regop_write_40(&mut self, reg: u32, val: u8) {
        self.regop_write(reg, val, operator_write_40);
    }

    fn regop_write_60(&mut self, reg: u32, val: u8) {
        self.regop_write(reg, val, operator_write_60);
    }

    fn regop_write_80(&mut self, reg: u32, val: u8) {
        self.regop_write(reg, val, operator_write_80);
    }

    fn regop_write_e0(&mut self, reg: u32, val: u8) {
        self.regop_write(reg, val, operator_write_e0);
    }

    fn regchan_write_a0(&mut self, reg: u32, val: u8) {
        let ix = ((reg >> 4) & 0x10) | (reg & 0xf);
        if let Some(offset) = self.tables.chan_offset_table[ix as usize] {
            self.channel_write_a0(offset, val);
        }
    }

    fn channel_write_a0(&mut self, offset: usize, val: u8) {
        let channels = if offset == (NUM_CHANNELS - 1) {
            &mut self.channels[offset..(offset + 1)]
        } else {
            &mut self.channels[offset..=(offset + 1)]
        };
        let four_op = self.reg_104 & self.opl3_active & channels[0].four_mask;
        //don't handle writes to silent fourop channels
        if four_op > 0x80 {
            return;
        }
        let change = (channels[0].chan_data ^ val as u32) & 0xff;
        if change != 0 {
            channels[0].chan_data ^= change;
            channel_update_frequency(channels, four_op, self.reg_08, &self.tables);
        }
    }

    fn regchan_write_b0(&mut self, reg: u32, val: u8) {
        let ix = ((reg >> 4) & 0x10) | (reg & 0xf);
        if let Some(offset) = self.tables.chan_offset_table[ix as usize] {
            self.channel_write_b0(offset, val);
        }
    }

    fn channel_write_b0(&mut self, offset: usize, val: u8) {
        let channels = if offset == (NUM_CHANNELS - 1) {
            &mut self.channels[offset..(offset + 1)]
        } else {
            &mut self.channels[offset..=(offset + 1)]
        };
        let four_op = self.reg_104 & self.opl3_active & channels[0].four_mask;
        //don't handle writes to silent fourop channels
        if four_op > 0x80 {
            return;
        }
        let change = (channels[0].chan_data ^ ((val as u32) << 8)) & 0x1f00;
        if change != 0 {
            channels[0].chan_data ^= change;
            channel_update_frequency(channels, four_op, self.reg_08, &self.tables);
        }

        //check for a change in the keyon/off state
        if ((val ^ channels[0].reg_b0) & 0x20) == 0 {
            return;
        }

        channels[0].reg_b0 = val;
        if (val & 0x20) != 0 {
            channels[0].op(0).key_on(0x1);
            channels[0].op(1).key_on(0x1);
            if (four_op & 0x3f) != 0 {
                channels[1].op(0).key_on(1);
                channels[1].op(1).key_on(1);
            }
        } else {
            channels[0].op(0).key_off(0x1);
            channels[0].op(1).key_off(0x1);
            if (four_op & 0x3f) != 0 {
                channels[1].op(0).key_off(1);
                channels[1].op(1).key_off(1);
            }
        }
    }

    fn regchan_write_c0(&mut self, reg: u32, val: u8) {
        let ix = ((reg >> 4) & 0x10) | (reg & 0xf);
        if let Some(offset) = self.tables.chan_offset_table[ix as usize] {
            self.channel_write_c0(offset, val);
        }
    }

    fn channel_write_c0(&mut self, offset: usize, val: u8) {
        let channel = &mut self.channels[offset];
        let change = val ^ channel.reg_c0;
        if change == 0 {
            return;
        }
        channel.reg_c0 = val;
        channel.feedback = (val >> 1) & 7;
        if channel.feedback != 0 {
            channel.feedback = 9 - channel.feedback;
        } else {
            channel.feedback = 31;
        }

        //TODO add OPL3 Support

        //disable updating percussion channels
        if (channel.four_mask & 0x40) != 0 && (self.reg_bd & 0x20) != 0 {
        } else if (val & 1) != 0 {
            channel.synth_handler = channel_block_template_sm2am;
        } else {
            channel.synth_handler = channel_block_template_sm2fm;
        }
    }

    fn channel_reset_c0(&mut self, offset: usize) {
        let val = self.channels[offset].reg_c0;
        self.channels[offset].reg_c0 ^= 0xff;
        self.channel_write_c0(offset, val);
    }

    fn generate_block_2(&mut self, total_in: usize, mix_buffer: &mut Vec<i32>) {
        mix_buffer.fill(0);

        let mut mix_offset = 0;
        let mut total = total_in;
        while total != 0 {
            let samples = self.forward_lfo(total as u32) as usize;
            let mut chan_ptr = 0;
            while chan_ptr < 9 {
                let chan = &mut self.channels[chan_ptr];
                let ch_shift =
                    (chan.synth_handler)(self, chan_ptr, samples, &mut mix_buffer[mix_offset..]);
                chan_ptr += ch_shift;
            }
            total -= samples;
            mix_offset += samples;
        }
    }

    fn forward_lfo(&mut self, samples: u32) -> u32 {
        //current vibrato value, runs 4x slower than tremolo
        self.vibrato_sign = VIBRATO_TABLE[(self.vibrato_index >> 2) as usize] >> 7;
        self.vibrato_shift =
            (VIBRATO_TABLE[(self.vibrato_index >> 2) as usize] & 7) as u8 + self.vibrato_strength;
        self.tremolo_value =
            self.tables.tremolo_table[self.tremolo_index as usize] >> self.tremolo_strength;

        //check how many samples there can be done before the value changes
        let todo = LFO_MAX - self.lfo_counter;
        let mut count = (todo + self.lfo_add - 1) / self.lfo_add;
        if count > samples {
            count = samples;
            self.lfo_counter += count * self.lfo_add;
        } else {
            self.lfo_counter += count * self.lfo_add;
            self.lfo_counter &= LFO_MAX - 1;
            //maximum of 7 vibrato value * 4
            self.vibrato_index = (self.vibrato_index + 1) & 31;
            //clip tremolo to the the table size
            if ((self.tremolo_index + 1) as usize) < TREMOLO_TABLE_SIZE {
                self.tremolo_index += 1;
            } else {
                self.tremolo_index = 0;
            }
        }
        count
    }
}

fn channel_update_frequency(channels: &mut [Channel], four_op: u8, reg_08: u8, tables: &Tables) {
    //extract the frequency bits
    let mut data = channels[0].chan_data & 0xffff;
    let ksl_base = tables.ksl_table[(data >> 6) as usize];
    let mut key_code = (data & 0x1c00) >> 9;
    if (reg_08 & 0x40) != 0 {
        key_code |= (data & 0x100) >> 8; /* notesel == 1 */
    } else {
        key_code |= (data & 0x200) >> 9; /* notesel == 0 */
    }
    //add the keycode and ksl into the highest bits of chanData
    data |= (key_code << SHIFT_KEYCODE) | ((ksl_base as u32) << SHIFT_KSLBASE);
    channels[0].set_chan_data(tables, data);
    if (four_op & 0x3f) != 0 {
        channels[1].set_chan_data(tables, data);
    }
}

// Operators

fn operator_write_20(op: &mut Operator, tables: &Tables, _: &ChipValues, val: u8) {
    let change = op.reg_20 ^ val;
    if change == 0 {
        return;
    }
    op.reg_20 = val;
    //shift the tremolo bit over the entire register, saved a branch, YES!
    op.tremolo_mask = val >> 7;
    op.tremolo_mask &= !((1 << ENV_EXTRA) - 1);
    //update specific features based on changes
    if (change & MASK_KSR) != 0 {
        operator_update_rates(op, tables);
    }
    //with sustain enable the volume doesn't change
    if (op.reg_20 & MASK_SUSTAIN) != 0 || op.release_add == 0 {
        op.rate_zero |= 1 << OperatorState::SUSTAIN as u8;
    } else {
        op.rate_zero &= !(1 << OperatorState::SUSTAIN as u8);
    }
    //frequency multiplier or vibrato changed
    if (change & (0xf | MASK_VIBRATO)) != 0 {
        op.freq_mul = tables.freq_mul[(val & 0xf) as usize];
        operator_update_frequency(op);
    }
}

fn operator_write_40(op: &mut Operator, _: &Tables, _: &ChipValues, val: u8) {
    if (op.reg_40 ^ val) == 0 {
        return;
    }
    op.reg_40 = val;
    operator_update_attenuation(op);
}

fn operator_write_60(op: &mut Operator, tables: &Tables, _: &ChipValues, val: u8) {
    let change = op.reg_60 ^ val;
    op.reg_60 = val;
    if (change & 0x0f) != 0 {
        operator_update_decay(op, tables);
    }
    if (change & 0xf0) != 0 {
        operator_update_attack(op, tables);
    }
}

fn operator_write_80(op: &mut Operator, tables: &Tables, _: &ChipValues, val: u8) {
    let change = op.reg_80 ^ val;
    if change == 0 {
        return;
    }
    op.reg_80 = val;
    let mut sustain = val >> 4;
    sustain |= (sustain + 1) & 0x10;
    op.sustain_level = (sustain as i32) << (ENV_BITS - 5);
    if (change & 0x0f) != 0 {
        operator_update_release(op, tables);
    }
}

fn operator_write_e0(op: &mut Operator, _: &Tables, chip: &ChipValues, val: u8) {
    if (op.reg_e0 ^ val) == 0 {
        return;
    }

    //in opl3 mode you can always selet 7 waveforms regardless of waveformselect
    let wave_form =
        (val & ((0x03 & chip.wave_form_mask) | (0x7 & chip.opl3_active as u8))) as usize;
    op.reg_e0 = val;
    // TODO #if( DBOPL_WAVE == WAVE_HANDLER ) ?
    op.wave_base = WAVE_BASE_TABLE[wave_form];
    op.wave_start = (WAVE_START_TABLE[wave_form] as u32) << WAVE_SH;
    op.wave_mask = WAVE_MASK_TABLE[wave_form] as u32;
}

fn operator_update_rates(op: &mut Operator, tables: &Tables) {
    let mut new_ksr = ((op.chan_data >> SHIFT_KEYCODE) & 0xff) as u8;
    if (op.reg_20 & MASK_KSR) == 0 {
        new_ksr >>= 2;
    }
    if op.ksr == new_ksr {
        return;
    }
    op.ksr = new_ksr;

    operator_update_attack(op, tables);
    operator_update_decay(op, tables);
    operator_update_release(op, tables);
}

fn operator_update_frequency(op: &mut Operator) {
    let freq = op.chan_data & ((1 << 10) - 1);
    let block = (op.chan_data >> 10) & 0xff;

    // TODO Impl WAVE_PRECSION mode here
    op.wave_add = (freq << block) * op.freq_mul;
    if (op.reg_20 & MASK_VIBRATO) != 0 {
        op.vib_strength = (freq >> 7) as u8;
        // TODO Impl WAVE_PRECSION mode also here
        op.vibrato = ((op.vib_strength as u32) << block) * op.freq_mul;
    } else {
        op.vib_strength = 0;
        op.vibrato = 0;
    }
}

fn operator_update_release(op: &mut Operator, tables: &Tables) {
    let rate = op.reg_80 & 0xf;
    if rate != 0 {
        let val = (rate << 2) + op.ksr;
        op.release_add = tables.linear_rates[val as usize];
        op.rate_zero &= !(1 << OperatorState::RELEASE as u8);
        if (op.reg_20 & MASK_SUSTAIN) == 0 {
            op.rate_zero &= !(1 << OperatorState::SUSTAIN as u8);
        }
    } else {
        op.rate_zero |= 1 << OperatorState::RELEASE as u8;
        op.release_add = 0;
        if (op.reg_20 & MASK_SUSTAIN) == 0 {
            op.rate_zero |= 1 << OperatorState::SUSTAIN as u8;
        }
    }
}

fn operator_update_decay(op: &mut Operator, tables: &Tables) {
    let rate = op.reg_60 & 0x0f;
    if rate != 0 {
        let val = (rate << 2) + op.ksr;
        op.decay_add = tables.linear_rates[val as usize];
        op.rate_zero &= !(1 << OperatorState::DECAY as u8);
    } else {
        op.decay_add = 0;
        op.rate_zero |= 1 << OperatorState::DECAY as u8;
    }
}

fn operator_update_attack(op: &mut Operator, tables: &Tables) {
    let rate = op.reg_60 >> 4;
    if rate != 0 {
        let val = (rate << 2) + op.ksr;
        op.attack_add = tables.attack_rates[val as usize];
        op.rate_zero &= !(1 << OperatorState::ATTACK as u8);
    } else {
        op.attack_add = 0;
        op.rate_zero |= 1 << OperatorState::ATTACK as u8;
    }
}

fn operator_update_attenuation(op: &mut Operator) {
    let ksl_base = ((op.chan_data >> SHIFT_KSLBASE) & 0xFF) as i32;
    let tl = (op.reg_40 & 0x3f) as i32;
    let ksl_shift = KSL_SHIFT_TABLE[(op.reg_40 >> 6) as usize];

    //make sure the attenuation goes to the right bits
    op.total_level = tl << ((ENV_BITS - 7) as u8); //total level goes 2 bits below max
    op.total_level += (ksl_base << ENV_EXTRA) >> ksl_shift;
}

fn operator_silent(op: &Operator) -> bool {
    if !env_silent(op.total_level + op.volume) {
        return false;
    }
    if (op.rate_zero & (1 << op.state as u8)) == 0 {
        return false;
    }
    true
}

fn operator_prepare(chip: &mut Chip, channel_ix: usize, op_ix: usize) {
    let op = chip.channels[channel_ix].op(op_ix);
    op.current_level = op.total_level + (chip.tremolo_value & op.tremolo_mask) as i32;
    op.wave_current = op.wave_add;
    if (op.vib_strength >> chip.vibrato_shift) != 0 {
        let mut add = (op.vibrato >> chip.vibrato_shift) as i32;
        //sign extend over the shift value
        let neg = chip.vibrato_sign as i32;
        //negate the add with -1 or 0
        add = (add ^ neg) - neg;
        op.wave_current = op.wave_current.wrapping_add_signed(add);
    }
}

fn operator_get_sample(op: &mut Operator, tables: &Tables, modulation: i32) -> i32 {
    let vol = operator_forward_volume(op);
    if env_silent(vol) {
        //simply forward the wave
        (op.wave_index, _) = op.wave_index.overflowing_add(op.wave_current);
        0
    } else {
        let mut index = operator_forward_wave(op) as i32;
        index += modulation;
        operator_get_wave(op, tables, index, vol)
    }
}

fn operator_get_wave(op: &mut Operator, tables: &Tables, index: i32, vol: i32) -> i32 {
    // TODO Impl DBOPL_WAVE == WAV_HANDLER and DBOP_WAVE == WAVE_TABLELOG
    // current impl is only DPOPL_WAVE == WAVE_TABLEMUL
    let wave = tables.wave_table[op.wave_base + (index & op.wave_mask as i32) as usize] as i32;
    let mul = tables.mul_table[(vol >> ENV_EXTRA) as usize] as i32;
    (wave * mul) >> MUL_SH
}

fn operator_forward_volume(op: &mut Operator) -> i32 {
    op.current_level + (op.vol_handler)(op)
}

fn operator_forward_wave(op: &mut Operator) -> u32 {
    (op.wave_index, _) = op.wave_index.overflowing_add(op.wave_current);
    op.wave_index >> WAVE_SH
}

fn operator_rate_forward(op: &mut Operator, add: u32) -> i32 {
    op.rate_index += add;
    let ret = op.rate_index >> RATE_SH;
    op.rate_index = op.rate_index & RATE_MASK;
    ret as i32
}

// Channel Block Templates

fn channel_block_template_sm2fm(
    chip: &mut Chip,
    channel_ix: usize,
    samples: usize,
    output: &mut [i32],
) -> usize {
    channel_block_template(chip, channel_ix, samples, output, SynthMode::SM2FM)
}

fn channel_block_template_sm2percussion(
    chip: &mut Chip,
    channel_ix: usize,
    samples: usize,
    output: &mut [i32],
) -> usize {
    channel_block_template(chip, channel_ix, samples, output, SynthMode::SM2Percussion)
}

fn channel_block_template_sm3percussion(
    chip: &mut Chip,
    channel_ix: usize,
    samples: usize,
    output: &mut [i32],
) -> usize {
    channel_block_template(chip, channel_ix, samples, output, SynthMode::SM3Percussion)
}

fn channel_block_template_sm2am(
    chip: &mut Chip,
    channel_ix: usize,
    samples: usize,
    output: &mut [i32],
) -> usize {
    channel_block_template(chip, channel_ix, samples, output, SynthMode::SM2AM)
}

fn channel_block_template(
    chip: &mut Chip,
    channel_ix: usize,
    samples: usize,
    output: &mut [i32],
    mode: SynthMode,
) -> usize {
    match mode {
        SynthMode::SM2AM | SynthMode::SM3AM => {
            if operator_silent(&chip.channels[channel_ix].operator[0])
                && operator_silent(&chip.channels[channel_ix].operator[1])
            {
                chip.channels[channel_ix].old[0] = 0;
                chip.channels[channel_ix].old[1] = 0;
                return 1;
            }
        }
        SynthMode::SM2FM | SynthMode::SM3FM => {
            if operator_silent(&chip.channels[channel_ix].operator[1]) {
                chip.channels[channel_ix].old[0] = 0;
                chip.channels[channel_ix].old[1] = 0;
                return 1;
            }
        }
        _ => todo!("block template {:?}", mode),
    }

    //init the operators with the the current vibrato and tremolo values
    operator_prepare(chip, channel_ix, 0);
    operator_prepare(chip, channel_ix, 1);

    if mode > SynthMode::SM4Start {
        operator_prepare(chip, channel_ix, 2);
        operator_prepare(chip, channel_ix, 3);
    }
    if mode > SynthMode::SM6Start {
        operator_prepare(chip, channel_ix, 4);
        operator_prepare(chip, channel_ix, 5);
    }

    for i in 0..samples {
        //early out for percussion handlers
        if mode == SynthMode::SM2Percussion {
            channel_generate_percussion(chip, channel_ix, &mut output[i..], false);
            continue; //prevent some unitialized value bitching
        } else if mode == SynthMode::SM3Percussion {
            channel_generate_percussion(chip, channel_ix, &mut output[(i * 2)..], true);
            continue; //prevent some unitialized value bitching
        }

        //do unsigned shift so we can shift out all bits but still stay in 10 bit range otherwise
        let modulation = ((chip.channels[channel_ix].old[0] + chip.channels[channel_ix].old[1])
            as u32
            >> chip.channels[channel_ix].feedback) as i32;
        chip.channels[channel_ix].old[0] = chip.channels[channel_ix].old[1];
        chip.channels[channel_ix].old[1] =
            operator_get_sample(chip.channels[channel_ix].op(0), &chip.tables, modulation);
        let mut sample = 0;
        let out_0 = chip.channels[channel_ix].old[0];
        if mode == SynthMode::SM2AM || mode == SynthMode::SM3AM {
            sample = out_0 + operator_get_sample(chip.channels[channel_ix].op(1), &chip.tables, 0);
        } else if mode == SynthMode::SM2FM || mode == SynthMode::SM3FM {
            sample = operator_get_sample(chip.channels[channel_ix].op(1), &chip.tables, out_0);
        } else if mode == SynthMode::SM3FMFM {
            let next = operator_get_sample(chip.channels[channel_ix].op(1), &chip.tables, out_0);
            let next = operator_get_sample(chip.channels[channel_ix].op(2), &chip.tables, next);
            sample = operator_get_sample(chip.channels[channel_ix].op(3), &chip.tables, next);
        } else if mode == SynthMode::SM3AMFM {
            sample = out_0;
            let next = operator_get_sample(chip.channels[channel_ix].op(1), &chip.tables, 0);
            let next = operator_get_sample(chip.channels[channel_ix].op(2), &chip.tables, next);
            sample += operator_get_sample(chip.channels[channel_ix].op(3), &chip.tables, next);
        } else if mode == SynthMode::SM3FMAM {
            sample = operator_get_sample(chip.channels[channel_ix].op(1), &chip.tables, out_0);
            let next = operator_get_sample(chip.channels[channel_ix].op(2), &chip.tables, 0);
            sample += operator_get_sample(chip.channels[channel_ix].op(3), &chip.tables, next);
        } else if mode == SynthMode::SM3AMAM {
            sample = out_0;
            let next = operator_get_sample(chip.channels[channel_ix].op(1), &chip.tables, 0);
            sample += operator_get_sample(chip.channels[channel_ix].op(2), &chip.tables, next);
            sample += operator_get_sample(chip.channels[channel_ix].op(3), &chip.tables, 0);
        }
        match mode {
            SynthMode::SM2AM | SynthMode::SM2FM => {
                output[i] += sample;
            }
            SynthMode::SM3AM
            | SynthMode::SM3FM
            | SynthMode::SM3FMFM
            | SynthMode::SM3AMFM
            | SynthMode::SM3FMAM
            | SynthMode::SM3AMAM => {
                output[i * 2 + 0] += sample & chip.channels[channel_ix].mask_left as i32;
                output[i * 2 + 1] += sample & chip.channels[channel_ix].mask_right as i32;
            }
            _ => { /*no-op*/ }
        }
    }

    match mode {
        SynthMode::SM2AM | SynthMode::SM2FM | SynthMode::SM3AM | SynthMode::SM3FM => 1,
        SynthMode::SM3FMFM | SynthMode::SM3AMFM | SynthMode::SM3FMAM | SynthMode::SM3AMAM => 2,
        SynthMode::SM2Percussion | SynthMode::SM3Percussion => 3,
        _ => 0,
    }
}

fn channel_generate_percussion(
    _chip: &mut Chip,
    _channel_ix: usize,
    _output: &mut [i32],
    _opl3_mode: bool,
) {
    todo!("channel_generate_percussion");
}

// Volume Templates

fn template_volume_off(op: &mut Operator) -> i32 {
    template_volume(op, OperatorState::OFF)
}

fn template_volume_release(op: &mut Operator) -> i32 {
    template_volume(op, OperatorState::RELEASE)
}

fn template_volume_sustain(op: &mut Operator) -> i32 {
    template_volume(op, OperatorState::SUSTAIN)
}

fn template_volume_attack(op: &mut Operator) -> i32 {
    template_volume(op, OperatorState::ATTACK)
}

fn template_volume_decay(op: &mut Operator) -> i32 {
    template_volume(op, OperatorState::DECAY)
}

fn template_volume(op: &mut Operator, state: OperatorState) -> i32 {
    let mut vol = op.volume;
    match state {
        OperatorState::OFF => {
            return ENV_MAX;
        }
        OperatorState::ATTACK => {
            let change = operator_rate_forward(op, op.attack_add);
            if change == 0 {
                return vol;
            }
            vol += ((!vol) * change) >> 3;
            if vol < ENV_MIN {
                op.volume = ENV_MIN;
                op.rate_index = 0;
                op.set_state(OperatorState::DECAY);
                return ENV_MIN;
            }
        }
        OperatorState::DECAY => {
            vol += operator_rate_forward(op, op.decay_add);
            if vol >= op.sustain_level {
                //check if we didn't overshoot max attenuation, then just go off
                if vol >= ENV_MAX {
                    op.volume = ENV_MAX;
                    op.set_state(OperatorState::OFF);
                    return ENV_MAX;
                }
                //continue as sustain
                op.rate_index = 0;
                op.set_state(OperatorState::SUSTAIN);
            }
        }
        OperatorState::SUSTAIN | OperatorState::RELEASE => {
            if state == OperatorState::SUSTAIN && (op.reg_20 & MASK_SUSTAIN) != 0 {
                return vol;
            }
            vol += operator_rate_forward(op, op.release_add);
            if vol >= ENV_MAX {
                op.volume = ENV_MAX;
                op.set_state(OperatorState::OFF);
                return ENV_MAX;
            }
        }
    }
    op.volume = vol;
    vol
}

// helper functions

fn env_silent(x: i32) -> bool {
    x >= ENV_LIMIT
}

// backend impl helper functions

pub fn adl_set_fx_inst(chip: &mut Chip, inst: &Instrument) {
    let c = 3;
    chip.write_reg(AL_CHAR, inst.m_char);
    chip.write_reg(AL_SCALE, inst.m_scale);
    chip.write_reg(AL_ATTACK, inst.m_attack);
    chip.write_reg(AL_SUS, inst.m_sus);
    chip.write_reg(AL_WAVE, inst.m_wave);
    chip.write_reg(c + AL_CHAR, inst.c_char);
    chip.write_reg(c + AL_SCALE, inst.c_scale);
    chip.write_reg(c + AL_ATTACK, inst.c_attack);
    chip.write_reg(c + AL_SUS, inst.c_sus);
    chip.write_reg(c + AL_WAVE, inst.c_wave);

    chip.write_reg(AL_FEED_CON, 0);
}
