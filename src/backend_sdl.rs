use core::borrow::BorrowMut;

use sdl2::audio::{AudioCallback, AudioDevice, AudioSpecDesired};
use sdl2::{self, AudioSubsystem};

use crate::{
    AdlSound, AdlState, Chip, ImfState, OPLSettings, AL_ATTACK, AL_CHAR, AL_SCALE, AL_SUS, AL_WAVE,
};

pub struct OPL {
    audio_subsystem: AudioSubsystem,
    device: Option<AudioDevice<OPLCallback>>,
}

// According to the SDL documentation the audio system is thread-safe.
// But the SDL API does not mark is as Send and without the 'Send' marker
// it is impossible to use this in an asynchronous context (as for example iron-wolf does).
unsafe impl Send for OPL {}

pub fn new() -> Result<OPL, &'static str> {
    let sdl_context = sdl2::init().expect("sdl init failed");
    let audio_subsystem = sdl_context.audio().expect("audio init failed");
    Ok(OPL {
        audio_subsystem,
        device: None,
    })
}

impl OPL {
    pub fn init(&mut self, settings: OPLSettings) {
        let desired_spec = AudioSpecDesired {
            freq: Some(settings.mixer_rate as i32),
            channels: Some(2),
            samples: Some(((settings.mixer_rate * 2048) / 44100) as u16),
        };

        let samples_per_music_tick = if settings.imf_clock_rate != 0 {
            settings.mixer_rate / settings.imf_clock_rate
        } else {
            settings.mixer_rate / 560
        };

        let adl_samples_per_tick = if settings.adl_clock_rate != 0 {
            settings.imf_clock_rate / settings.adl_clock_rate
        } else {
            settings.imf_clock_rate / 140
        };

        let device = self
            .audio_subsystem
            .open_playback(None, &desired_spec, |_| {
                // initialize the audio callback
                OPLCallback {
                    mix_buffer: vec![0; settings.mixer_rate as usize],
                    num_ready_samples: 0,
                    samples_per_music_tick,
                    adl_samples_per_tick,
                    chip: Chip::new(settings.mixer_rate),
                    imf_state: None,
                    adl_state: None,
                }
            })
            .expect("playback open failed");
        self.device = Some(device);
    }

    pub fn play_imf(&mut self, data: Vec<u8>) -> Result<(), &'static str> {
        self.assert_device();

        let device = self.device.as_mut().expect("device");
        {
            let mut cb = device.lock();
            let hack_len = data.len();
            cb.imf_state = Some(ImfState {
                data,
                hack_len,
                hack_seq_len: hack_len,
                hack_time: 0,
                al_time_count: 0,
                hack_ptr: 0,
            });
            cb.chip.setup();
        }

        device.resume();
        Ok(())
    }

    pub fn play_adl(&mut self, sound: AdlSound) -> Result<(), &'static str> {
        self.assert_device();

        let device = self.device.as_mut().expect("device");
        {
            let mut cb = device.lock();
            cb.adl_state = Some(AdlState {
                sound,
                data_ptr: 0,
                note: false,
            })
        }

        device.resume();
        Ok(())
    }

    pub fn write_reg(&mut self, reg: u32, val: u8) {
        self.assert_device();
        let device = self.device.as_mut().expect("device");
        let mut cb = device.lock();
        cb.chip.write_reg(reg, val);
    }

    fn assert_device(&self) {
        if self.device.is_none() {
            panic!("OPL not initialized, did you call init()?");
        }
    }
}

struct OPLCallback {
    mix_buffer: Vec<i32>,
    num_ready_samples: u32,
    samples_per_music_tick: u32,
    adl_samples_per_tick: u32,

    chip: Chip,
    imf_state: Option<ImfState>,
    adl_state: Option<AdlState>,
}

impl AudioCallback for OPLCallback {
    type Channel = i16;

    fn callback(&mut self, out: &mut [i16]) {
        let mut samples_len = out.len() as u32 >> 1;
        let mut out_offset = 0;

        if let Some(imf_state) = self.imf_state.borrow_mut() {
            loop {
                if self.num_ready_samples > 0 {
                    if self.num_ready_samples < samples_len {
                        opl_update(
                            &mut self.chip,
                            out,
                            out_offset,
                            self.num_ready_samples as usize,
                            &mut self.mix_buffer,
                        );
                        out_offset += self.num_ready_samples as usize * 2;
                        samples_len -= self.num_ready_samples;
                    } else {
                        opl_update(
                            &mut self.chip,
                            out,
                            out_offset,
                            samples_len as usize,
                            &mut self.mix_buffer,
                        );
                        self.num_ready_samples -= samples_len;
                        //return; //wait for next callback
                        break;
                    }
                }

                loop {
                    if imf_state.hack_time > imf_state.al_time_count {
                        break;
                    }

                    let t = u16::from_le_bytes(
                        imf_state.data[(imf_state.hack_ptr + 2)..(imf_state.hack_ptr + 4)]
                            .try_into()
                            .unwrap(),
                    ) as u32;
                    imf_state.hack_time = imf_state.al_time_count + t;

                    let reg = imf_state.data[imf_state.hack_ptr] as u32;
                    let val = imf_state.data[imf_state.hack_ptr + 1];

                    self.chip.write_reg(reg, val);
                    imf_state.hack_ptr += 4;
                    imf_state.hack_len -= 4;

                    if imf_state.hack_len <= 0 {
                        break;
                    }
                }
                imf_state.al_time_count += 1;
                if imf_state.hack_len == 0 {
                    imf_state.hack_ptr = 0;
                    imf_state.hack_len = imf_state.hack_seq_len;
                    imf_state.hack_time = 0;
                    imf_state.al_time_count = 0;
                }

                self.num_ready_samples = self.samples_per_music_tick;
            }
        }

        // TODO compute frequency from imf_freq and adl_freq relation
        for _ in 0..self.adl_samples_per_tick {
            if self.adl_state.is_some() {
                if adl_mixer(&mut self.chip, self.adl_state.as_mut().expect("adl state")) {
                    self.adl_state = None;
                    self.chip.write_reg(0xB0, 0); // write silence at the end so that last note does not repeat
                }
            }
        }
    }
}

// Returns 'true' if the sound was played to the end.
fn adl_mixer(chip: &mut Chip, state: &mut AdlState) -> bool {
    if state.data_ptr == 0 {
        let c = 3;
        chip.write_reg(AL_CHAR, state.sound.instrument.m_char);
        chip.write_reg(AL_SCALE, state.sound.instrument.m_scale);
        chip.write_reg(AL_ATTACK, state.sound.instrument.m_attack);
        chip.write_reg(AL_SUS, state.sound.instrument.m_sus);
        chip.write_reg(AL_WAVE, state.sound.instrument.m_wave);
        chip.write_reg(c + AL_CHAR, state.sound.instrument.c_char);
        chip.write_reg(c + AL_SCALE, state.sound.instrument.c_scale);
        chip.write_reg(c + AL_ATTACK, state.sound.instrument.c_attack);
        chip.write_reg(c + AL_SUS, state.sound.instrument.c_sus);
        chip.write_reg(c + AL_WAVE, state.sound.instrument.c_wave);
    }

    let note = state.sound.data[state.data_ptr];
    state.data_ptr += 1;

    if note == 0 {
        chip.write_reg(0xB0, 0);
        state.note = false;
    } else {
        if !state.note {
            let block = ((state.sound.block & 7) << 2) | 0x20;
            chip.write_reg(0xB0, block);
        }
        state.note = true;
        chip.write_reg(0xA0, note);
    }

    return state.data_ptr >= state.sound.data.len();
}

fn opl_update(
    chip: &mut Chip,
    sdl_out: &mut [i16],
    offset: usize,
    len: usize,
    mix_buffer: &mut Vec<i32>,
) {
    chip.generate_block_2(len, mix_buffer);

    let mut mix_ptr = 0;
    let mut out_ptr = offset;
    for _ in 0..len {
        let mix = (mix_buffer[mix_ptr] << 2) as i16; // increase volume a bit
        mix_ptr += 1;

        sdl_out[out_ptr] = mix;
        out_ptr += 1;
        sdl_out[out_ptr] = mix;
        out_ptr += 1;
    }
}
