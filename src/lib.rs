#![cfg_attr(not(feature = "sdl"), no_std)]

#[cfg(test)]
#[path = "./lib_test.rs"]
mod lib_test;

use core::array::from_fn;

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

static FREQ_CREATE_TABLE: [u8; 16] = [1, 2, 4, 6, 8, 10, 12, 14, 16, 18, 20, 20, 24, 24, 30, 30];
static ATTACK_SAMPLES_TABLE: [u8; 13] = [69, 55, 46, 40, 35, 29, 23, 20, 19, 15, 11, 10, 9];
static ENVELOPE_INCREASE_TABLE: [u8; 13] = [4, 5, 6, 7, 8, 10, 12, 14, 16, 20, 24, 28, 32];

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

pub struct Chip {
    channels: [Channel; NUM_CHANNELS],

    //this is used as the base counter for vibrato and tremolo
    lfo_counter: u32,
    lfo_add: u32,

    reg_bd: u8,
    vibrato_index: u8,
    tremolo_index: u8,
    vibrato_sign: i8,
    vibrato_shift: u8,
    tremolo_value: u8,
    vibrato_strength: u8,
    tremolo_strength: u8,
    tables: Tables,
}

#[repr(u8)]
#[derive(Debug, Copy, Clone)]
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

    wave_add: u32,

    chan_data: u32,
    freq_mul: u32,
    vibrato: u32,
    sustain_level: i32,
    total_level: i32,
    volume: i32,

    attack_add: u32,
    decay_add: u32,
    release_add: u32,

    rate_zero: u8,
    reg_20: u8,
    reg_40: u8,
    reg_60: u8,
    reg_80: u8,
    state: OperatorState,
    tremolo_mask: u8,
    vib_strength: u8,
    ksr: u8,
}

impl Operator {
    pub fn new() -> Operator {
        Operator {
            vol_handler: template_volume_off,

            wave_add: 0,

            chan_data: 0,
            freq_mul: 0,
            vibrato: 0,
            sustain_level: ENV_MAX,
            total_level: ENV_MAX,
            volume: ENV_MAX,

            attack_add: 0,
            decay_add: 0,
            release_add: 0,

            rate_zero: 1 << (OperatorState::OFF as u8),
            reg_20: 0,
            reg_40: 0,
            reg_60: 0,
            reg_80: 0,
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
    self->keyOn = 0;
    self->regE0 = 0;
    Operator__SetState( self, OFF );
    self->currentLevel = ENV_MAX;
    self->volume = ENV_MAX;
    }
    */
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
    old: [i32; 2],

    feedback: u8,
    reg_c0: u8,

    //this should correspond with reg104, bit 6 indicates a Percussion channel, bit 7 indicates a silent channel
    four_mask: u8,
}

impl Channel {
    pub fn new() -> Channel {
        Channel {
            operator: [Operator::new(), Operator::new()],
            old: [0, 0],
            feedback: 31,
            reg_c0: 0,
            four_mask: 0,
            synth_handler: channel_block_template_sm2fm,
        }

        /*
        self->chanData = 0;
        self->regB0 = 0;
        self->maskLeft = -1;
        self->maskRight = -1;
        */
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
}

fn init_tables(rate: u32) -> Tables {
    let scale = OPL_RATE / rate as f64;

    // TODO Impl WAVE_PRECISION
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
        Chip {
            channels,
            lfo_counter: 0,
            lfo_add: 0,
            reg_bd: 0,
            vibrato_index: 0,
            tremolo_index: 0,
            vibrato_sign: 0,
            vibrato_shift: 0,
            tremolo_value: 0,
            vibrato_strength: 0,
            tremolo_strength: 0,
            tables: init_tables(rate),
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
            0xb0 => {
                if reg == 0xbd {
                    self.write_bd(val);
                } else {
                    todo!("REGCHAN(WriteB0)")
                }
            }
            0xc0 => self.regchan_write_c0(reg, val),
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
        f: fn(op: &mut Operator, tables: &Tables, val: u8),
    ) {
        let ix = ((reg >> 3) & 0x20) | (reg & 0x1f);
        if let Some(offset) = &self.tables.op_offset_table[ix as usize] {
            let op = &mut self.channels[offset.chan].operator[offset.op];
            f(op, &self.tables, val);
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

// Operators

fn operator_write_20(op: &mut Operator, tables: &Tables, val: u8) {
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
        operator_update_frequency(op, tables);
    }
}

fn operator_write_40(op: &mut Operator, _: &Tables, val: u8) {
    if (op.reg_40 ^ val) == 0 {
        return;
    }
    op.reg_40 = val;
    operator_update_attenuation(op);
}

fn operator_write_60(op: &mut Operator, tables: &Tables, val: u8) {
    let change = op.reg_60 ^ val;
    op.reg_60 = val;
    if (change & 0x0f) != 0 {
        operator_update_decay(op, tables);
    }
    if (change & 0xf0) != 0 {
        operator_update_attack(op, tables);
    }
}

fn operator_write_80(op: &mut Operator, tables: &Tables, val: u8) {
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

fn operator_update_frequency(op: &mut Operator, tables: &Tables) {
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

fn template_volume(op: &Operator, state: OperatorState) {
    todo!("impl template volume {:?}", state)
}

// helper functions

fn env_silent(x: i32) -> bool {
    x >= ENV_LIMIT
}
