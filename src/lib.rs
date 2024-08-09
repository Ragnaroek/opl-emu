#![cfg_attr(not(feature = "sdl"), no_std)]

#[cfg(test)]
#[path = "./lib_test.rs"]
mod lib_test;

use core::array::from_fn;
use std::f64::consts::PI;

#[cfg(feature = "sdl")]
pub mod backend_sdl;

#[cfg(feature = "sdl")]
pub fn new() -> Result<backend_sdl::OPL, &'static str> {
    return backend_sdl::new();
}

pub struct OPLSettings {
    pub mixer_rate: u32,
    pub imf_clock_rate: u32,
}

const OPL_RATE: f64 = 14318180.0 / 288.0;

const TREMOLO_TABLE_SIZE: usize = 52;

const NUM_CHANNELS: usize = 9;

// TODO impl WAVE_PRECISION WAVE_BITS mode
const WAVE_BITS: u32 = 10;
const WAVE_SH: u32 = 32 - WAVE_BITS;
const WAVE_MASK: u32 = (1 << WAVE_SH) - 1;

const LFO_SH: u32 = WAVE_SH - 10;
const LFO_MAX: u32 = 256 << LFO_SH;

const ENV_BITS: i32 = 9;
const ENV_EXTRA: i32 = ENV_BITS - 9;
const ENV_MAX: i32 = 511 << ENV_EXTRA;
const ENV_LIMIT: i32 = (12 * 256) >> (3 - ENV_EXTRA);

const RATE_SH: u32 = 24;
const RATE_MASK: u32 = (1 << RATE_SH) - 1;

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

static VOLUME_HANDLER_TABLE: [VolumeHandler; 5] = [
    template_volume_off,
    template_volume_release,
    template_volume_sustain,
    template_volume_decay,
    template_volume_attack,
];

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
    opl3_active: bool,

    tables: Tables,
}

pub struct ChipValues {
    wave_form_mask: u8,
    opl3_active: bool,
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

type VolumeHandler = fn(channel: &Operator);

pub struct Operator {
    vol_handler: VolumeHandler,

    // TODO #if (DBOPL_WAVE == WAVE_HANDLER) ?
    wave_base: i16,
    wave_mask: u32,
    wave_start: u32,
    wave_add: u32,
    wave_index: u32,

    chan_data: u32,
    freq_mul: u32,
    vibrato: u32,
    sustain_level: i32,
    total_level: i32,
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
        Operator {
            vol_handler: template_volume_off,

            wave_base: 0,
            wave_mask: 0,
            wave_start: 0,
            wave_add: 0,
            wave_index: 0,

            chan_data: 0,
            freq_mul: 0,
            vibrato: 0,
            sustain_level: ENV_MAX,
            total_level: ENV_MAX,
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
        }
    }
    /*
    static void Operator__Operator(Operator *self) {
    self->freqMul = 0;
    self->waveIndex = 0;
    self->waveCurrent = 0;
    Operator__SetState( self, OFF );
    self->currentLevel = ENV_MAX;
    self->volume = ENV_MAX;
    }
    */

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

#[derive(Debug)]
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
    fn(channel: &mut Channel, samples: usize, output: &mut Vec<i32>, output_offset: usize) -> usize;

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
            synth_handler: channel_block_template_sm2fm,
        }

        /*
        self->maskLeft = -1;
        self->maskRight = -1;
        */
    }

    pub fn op(&mut self, ix: usize) -> &mut Operator {
        &mut self.operator[ix]
    }

    fn set_chan_data(&mut self, data: u32) {
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
            operator_update_attenuation(self.op(0));
            operator_update_attenuation(self.op(1));
        }
    }
}

struct OpOffset {
    chan: usize,
    op: usize,
}

struct Tables {
    chan_offset_table: [usize; NUM_CHANNELS],
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
    ksl_table: [u8; 8 * 16],
}

fn init_tables(scale: f64) -> Tables {
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
    println!("freq_scale={}, scale={}", freq_scale, scale);
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
        //panic!("stop")
    }
    for i in 62..76 {
        attack_rates[i] = 8 << RATE_SH;
    }

    let mut chan_offset_table = [0; 9];
    for i in 0..NUM_CHANNELS {
        let mut index = i & 0xf;
        //Make sure the four op channels follow eachother
        if index < 6 {
            index = (index % 3) * 2 + (index / 3);
        }
        chan_offset_table[i] = index;
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
        Some(OpOffset {
            chan: ch_num,
            op: op_num,
        })
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
            opl3_active: false,
            tables: init_tables(scale),
        }
    }

    pub fn write_reg(&mut self, reg: u32, val: u8) {
        println!("## reg = {:x}, val = {:x}", reg, val);
        match reg & 0xf0 {
            0x00 => {}
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
            0xe0 | 0xf0 => self.regop_write_e0(reg, val),
            _ => todo!("reg {:x} not implemented", reg),
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

        todo!("write bd val {:x}", val);
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
        let offset = self.tables.chan_offset_table[ix as usize];
        self.channel_write_a0(offset, val);
    }

    fn channel_write_a0(&mut self, offset: usize, val: u8) {
        let channels = if offset == 8 {
            &mut self.channels[offset..(offset + 1)]
        } else {
            &mut self.channels[offset..=(offset + 1)]
        };
        let four_op = self.reg_104 & if self.opl3_active { 1 } else { 0 } & channels[0].four_mask;
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
        let offset = self.tables.chan_offset_table[ix as usize];
        self.channel_write_b0(offset, val);
    }

    fn channel_write_b0(&mut self, offset: usize, val: u8) {
        let channels = if offset == 8 {
            &mut self.channels[offset..(offset + 1)]
        } else {
            &mut self.channels[offset..=(offset + 1)]
        };
        let four_op = self.reg_104 & if self.opl3_active { 1 } else { 0 } & channels[0].four_mask;
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
        if (!((val ^ channels[0].reg_b0) & 0x20)) != 0 {
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
        let offset = self.tables.chan_offset_table[ix as usize];
        self.channel_write_c0(offset, val);
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

    fn generate_block_2(&mut self, total_in: usize, mix_buffer: &mut Vec<i32>) {
        mix_buffer.fill(0);

        let mut mix_offset = 0;
        let mut total = total_in;
        while total != 0 {
            let samples = self.forward_lfo(total as u32) as usize;
            let mut chan_ptr = 0;
            while chan_ptr < NUM_CHANNELS {
                let chan = &mut self.channels[chan_ptr];
                let ch_shift = (chan.synth_handler)(chan, samples, mix_buffer, mix_offset);
                chan_ptr += ch_shift;
            }
            total -= samples;
            mix_offset += samples;
        }
    }

    fn forward_lfo(&mut self, samples: u32) -> u32 {
        // current vibrato value, runs 4x slower than tremolo
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
    channels[0].set_chan_data(data);
    if (four_op & 0x3f) != 0 {
        channels[1].set_chan_data(data);
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
    op.reg_80 = change;
    let mut sustain = val >> 4;
    sustain |= (sustain + 1) & 0x10;
    op.sustain_level = (sustain as i32) << (ENV_BITS - 5);
    if (change & 0x0f) != 0 {
        operator_update_release(op, tables);
    }
}

fn operator_write_e0(op: &mut Operator, tables: &Tables, chip: &ChipValues, val: u8) {
    if (op.reg_e0 ^ val) == 0 {
        return;
    }

    //in opl3 mode you can always selet 7 waveforms regardless of waveformselect
    let wave_form =
        (val & ((0x03 & chip.wave_form_mask) | (0x7 & chip.opl3_active as u8))) as usize;
    op.reg_e0 = val;
    // TODO #if( DBOPL_WAVE == WAVE_HANDLER ) ?
    op.wave_base = tables.wave_table[WAVE_BASE_TABLE[wave_form]];
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
            op.rate_zero &= 1 << OperatorState::SUSTAIN as u8;
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
        op.rate_zero = !(1 << OperatorState::ATTACK as u8);
    } else {
        op.attack_add = 0;
        op.rate_zero |= 1 << OperatorState::ATTACK as u8;
    }
}

fn operator_update_attenuation(op: &mut Operator) {
    let ksl_base = ((op.chan_data >> SHIFT_KSLBASE) & 0xFF) as i32;
    let tl = op.reg_40 & 0x3f;
    let ksl_shift = KSL_SHIFT_TABLE[(op.reg_40 >> 6) as usize];

    //make sure the attenuation goes to the right bits
    op.total_level = (tl << ((ENV_BITS - 7) as u8)) as i32; //Total level goes 2 bits below max
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

// Channel Block Templates

fn channel_block_template_sm2fm(
    channel: &mut Channel,
    samples: usize,
    output: &mut Vec<i32>,
    output_offset: usize,
) -> usize {
    channel_block_template(channel, samples, output, output_offset, SynthMode::SM2FM)
}

fn channel_block_template_sm2am(
    channel: &mut Channel,
    samples: usize,
    output: &mut Vec<i32>,
    output_offset: usize,
) -> usize {
    channel_block_template(channel, samples, output, output_offset, SynthMode::SM2AM)
}

fn channel_block_template(
    channel: &mut Channel,
    samples: usize,
    output: &mut Vec<i32>,
    output_offset: usize,
    mode: SynthMode,
) -> usize {
    match mode {
        SynthMode::SM2AM | SynthMode::SM3AM => {
            if operator_silent(&channel.operator[0]) && operator_silent(&channel.operator[1]) {
                channel.old[0] = 0;
                channel.old[1] = 0;
                return 1;
            }
        }
        SynthMode::SM2FM | SynthMode::SM3FM => {
            if operator_silent(&channel.operator[1]) {
                channel.old[0] = 0;
                channel.old[1] = 0;
                return 1;
            }
        }
        _ => todo!("block template {:?}", mode),
    }

    todo!("block template handling {:?}", mode);
    return 0;
}

// Volume Templates

fn template_volume_off(op: &Operator) {
    template_volume(op, OperatorState::OFF)
}

fn template_volume_release(op: &Operator) {
    template_volume(op, OperatorState::RELEASE)
}

fn template_volume_sustain(op: &Operator) {
    template_volume(op, OperatorState::SUSTAIN)
}

fn template_volume_attack(op: &Operator) {
    template_volume(op, OperatorState::ATTACK)
}

fn template_volume_decay(op: &Operator) {
    template_volume(op, OperatorState::DECAY)
}

fn template_volume(op: &Operator, state: OperatorState) {
    todo!("impl template volume {:?}", state)
}

// helper functions

fn env_silent(x: i32) -> bool {
    x >= ENV_LIMIT
}
