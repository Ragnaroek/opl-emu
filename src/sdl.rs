use core::borrow::BorrowMut;

use sdl2::audio::{AudioCallback, AudioDevice, AudioSpecDesired};
use sdl2::{self, AudioSubsystem};

use crate::chip::{AL_FREQ_H, AL_FREQ_L, AdlSound, Chip, adl_set_fx_inst};

pub struct OPL {
    audio_subsystem: AudioSubsystem,
    device: Option<AudioDevice<OPLCallback>>,
}

pub struct OPLSettings {
    pub mixer_rate: u32,
    pub imf_clock_rate: u32,
    pub adl_clock_rate: u32,
}

struct SdlImfState {
    pub data: Vec<u8>,

    pub hack_ptr: usize,
    pub hack_len: usize,
    pub hack_seq_len: usize,
    pub hack_time: u32,
    pub al_time_count: u32,
    pub sq_active: bool,
}

struct SdlAdlState {
    pub sound: AdlSound,
    pub data_ptr: usize,
    pub sound_time_counter: u32,
    pub al_block: u8,
}

// According to the SDL documentation the audio system is thread-safe.
// But the SDL API does not mark is as Send and without the 'Send' marker
// it is impossible to use this in an asynchronous context (as for example iron-wolf does).
unsafe impl Send for OPL {}

impl OPL {
    pub fn new() -> Result<OPL, &'static str> {
        let sdl_context = sdl2::init().expect("sdl init failed");
        let audio_subsystem = sdl_context.audio().expect("audio init failed");
        Ok(OPL {
            audio_subsystem,
            device: None,
        })
    }

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
        self.assert_device()?;

        let device = self.mut_device()?;
        {
            let mut cb = device.lock();
            let hack_len = data.len();
            cb.imf_state = Some(SdlImfState {
                data,
                hack_len,
                hack_seq_len: hack_len,
                hack_time: 0,
                al_time_count: 0,
                hack_ptr: 0,
                sq_active: true,
            });
            cb.chip.setup();
        }
        device.resume();
        Ok(())
    }

    pub fn stop_imf(&mut self) -> Result<(), &'static str> {
        self.assert_device()?;
        let device = self.mut_device()?;
        {
            let mut cb = device.lock();
            if let Some(imf_state) = &mut cb.imf_state {
                imf_state.sq_active = false;
            }
        }
        Ok(())
    }

    pub fn pause_imf(&mut self) -> Result<(), &'static str> {
        self.assert_device()?;

        self.mut_device()?.pause();
        Ok(())
    }

    pub fn play_adl(&mut self, sound: AdlSound) -> Result<(), &'static str> {
        self.assert_device()?;

        let device = self.mut_device()?;
        {
            let mut cb = device.lock();
            adl_set_fx_inst(&mut cb.chip, &sound.instrument);
            let al_block = ((sound.block & 7) << 2) | 0x20;
            cb.adl_state = Some(SdlAdlState {
                sound,
                data_ptr: 0,
                al_block,
                sound_time_counter: cb.adl_samples_per_tick,
            })
        }

        device.resume();
        Ok(())
    }

    pub fn stop_adl(&mut self) -> Result<(), &'static str> {
        self.assert_device()?;
        let device = self.mut_device()?;
        device.pause();
        let mut cb = device.lock();
        cb.adl_state = None;
        Ok(())
    }

    pub fn is_adl_playing(&mut self) -> Result<bool, &'static str> {
        self.assert_device()?;
        let device = self.mut_device()?;
        let cb = device.lock();
        Ok(cb.adl_state.is_some())
    }

    pub fn is_imf_playing(&mut self) -> Result<bool, &'static str> {
        self.assert_device()?;
        let device = self.mut_device()?;
        let cb = device.lock();
        Ok(cb.imf_state.is_some())
    }

    pub fn write_reg(&mut self, reg: u32, val: u8) -> Result<(), &'static str> {
        self.assert_device()?;

        let device = self.mut_device()?;
        let mut cb = device.lock();
        cb.chip.write_reg(reg, val);
        Ok(())
    }

    fn assert_device(&self) -> Result<(), &'static str> {
        if self.device.is_none() {
            return Err("OPL not initialized, did you call init()?");
        }
        Ok(())
    }

    fn mut_device(&mut self) -> Result<&mut AudioDevice<OPLCallback>, &'static str> {
        let may_device = self.device.as_mut();
        if let Some(device) = may_device {
            Ok(device)
        } else {
            Err("no device")
        }
    }
}

struct OPLCallback {
    mix_buffer: Vec<i32>,
    num_ready_samples: u32,
    samples_per_music_tick: u32,
    adl_samples_per_tick: u32,

    chip: Chip,
    imf_state: Option<SdlImfState>,
    adl_state: Option<SdlAdlState>,
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
                        break;
                    }
                }

                if self.adl_state.is_some() {
                    let state = self.adl_state.as_mut().expect("adl state");
                    state.sound_time_counter -= 1;
                    if state.sound_time_counter == 0 {
                        state.sound_time_counter = self.adl_samples_per_tick;
                        if state.data_ptr < state.sound.data.len() {
                            let al_sound = state.sound.data[state.data_ptr];
                            if al_sound != 0 {
                                self.chip.write_reg(AL_FREQ_L, al_sound);
                                self.chip.write_reg(AL_FREQ_H, state.al_block);
                            } else {
                                self.chip.write_reg(AL_FREQ_H, 0);
                            }
                            state.data_ptr += 1;
                        } else {
                            self.adl_state = None;
                            self.chip.write_reg(AL_FREQ_H, 0); // write silence at the end so that last note does not repeat
                        }
                    }
                }

                if imf_state.sq_active {
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

                        if imf_state.hack_len == 0 {
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
                }
                self.num_ready_samples = self.samples_per_music_tick;
            }
        }
    }
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
