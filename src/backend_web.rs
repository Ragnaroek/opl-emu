use js_sys::Float32Array;
use std::cell::RefCell;
use std::rc::Rc;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;
use web_sys::{AudioContext, AudioWorkletNode, AudioWorkletNodeOptions, window};

use crate::{
    AL_FREQ_H, AL_FREQ_L, AdlSound, AdlState, Chip, ImfState, OPLSettings, adl_set_fx_inst,
};

pub struct OPL {
    audio_ctx: AudioContext,
    node: Rc<AudioWorkletNode>,
    state: Rc<RefCell<Option<PlaybackState>>>,
}

struct PlaybackState {
    num_ready_samples: u32,
    samples_per_music_tick: u32,
    adl_samples_per_tick: u32,
    chip: Chip,
    mix_buffer: Vec<i32>,
    imf_state: Option<ImfState>,
    adl_state: Option<AdlState>,
}

const BLOCK_SIZE: u32 = 14 * 128; // should be a multiple of 256
const FLOAT_CONVERSION_FACTOR: f32 = 32768.0;

impl PlaybackState {
    fn update_audio(&mut self, node: &AudioWorkletNode) -> Result<(), JsValue> {
        let mut block = Float32Array::new_with_length(BLOCK_SIZE);
        self.generate_block(&mut block);

        // send generated block to the audio processor
        node.port().unwrap().post_message(&JsValue::from(block))?;
        Ok(())
    }

    fn generate_block(&mut self, out: &mut Float32Array) {
        let mut samples_len = out.length() >> 1;
        let mut out_offset = 0;

        if self.imf_state.is_some() {
            loop {
                if self.num_ready_samples > 0 {
                    if self.num_ready_samples < samples_len {
                        self.opl_update(out, out_offset, self.num_ready_samples as usize);
                        out_offset += self.num_ready_samples * 2;
                        samples_len -= self.num_ready_samples;
                    } else {
                        self.opl_update(out, out_offset, samples_len as usize);
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

                let imf_state = self.imf_state.as_mut().expect("imf state");
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

    fn opl_update(&mut self, out: &mut Float32Array, offset: u32, len: usize) {
        self.chip.generate_block_2(len, &mut self.mix_buffer);

        let mut mix_ptr = 0;
        let mut out_ptr = offset;
        for _ in 0..len {
            let mix = (self.mix_buffer[mix_ptr] << 2) as i16; // increase volume a bit
            mix_ptr += 1;

            let f32_mix_val = mix as f32 / FLOAT_CONVERSION_FACTOR;
            out.set_index(out_ptr, f32_mix_val);
            out_ptr += 1;
            out.set_index(out_ptr, f32_mix_val);
            out_ptr += 1;
        }
    }
}

impl OPL {
    pub async fn new() -> Result<OPL, &'static str> {
        let audio_ctx = AudioContext::new().map_err(|_| "err init AudioContext")?;
        let worklet = audio_ctx
            .audio_worklet()
            .map_err(|_| "err getting audio worklet")?;
        let module_add = worklet
            .add_module("oplProcessor.js")
            .map_err(|_| "err start oplProcessor.js")?;
        JsFuture::from(module_add)
            .await
            .map_err(|_| "err adding oplProcessor.js")?;

        let options = AudioWorkletNodeOptions::new();
        options.set_number_of_outputs(1);
        options.set_output_channel_count(&js_sys::Array::of1(&2.into()));

        let node = AudioWorkletNode::new_with_options(&audio_ctx, "opl-processor", &options)
            .map_err(|_| "err creating AudioWorkletNode")?;
        node.connect_with_audio_node(&audio_ctx.destination())
            .map_err(|_| "err connecting with audio node")?;

        Ok(OPL {
            audio_ctx,
            node: Rc::new(node),
            state: Rc::new(RefCell::new(None)),
        })
    }

    pub async fn init(&mut self, settings: OPLSettings) -> Result<(), String> {
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

        let chip = Chip::new(settings.mixer_rate);
        let mix_buffer = vec![0; settings.mixer_rate as usize / 60]; // ~60 fps buffer

        let state = PlaybackState {
            chip,
            mix_buffer,
            samples_per_music_tick,
            adl_samples_per_tick,
            num_ready_samples: 0,
            imf_state: None,
            adl_state: None,
        };

        *RefCell::borrow_mut(&mut self.state) = Some(state);

        JsFuture::from(self.audio_ctx.resume().map_err(|_| "resume failed")?)
            .await
            .map_err(|_| "failed to resume audio context")?;

        // kick-off update-audio loop
        let mut state_timer = self.state.clone();
        let node_timer = self.node.clone();
        let closure = Closure::<dyn FnMut()>::new(move || {
            let mut state_timer = RefCell::borrow_mut(&mut state_timer);
            state_timer
                .as_mut()
                .expect("playback init")
                .update_audio(&node_timer)
                .expect("update audio");
        });

        window()
            .expect("browser windows")
            .set_interval_with_callback_and_timeout_and_arguments_0(
                closure.as_ref().unchecked_ref(),
                16,
            )
            .unwrap();
        closure.forget();

        Ok(())
    }

    pub fn play_imf(&mut self, data: Vec<u8>) -> Result<(), &'static str> {
        self.clear_buffer()?;

        let mut state_ref = RefCell::borrow_mut(&mut self.state);
        let state = state_ref.as_mut().expect("playback init");

        let hack_len = data.len();
        state.imf_state = Some(ImfState {
            data,
            hack_len,
            hack_seq_len: hack_len,
            hack_time: 0,
            al_time_count: 0,
            hack_ptr: 0,
            sq_active: true,
        });
        state.chip.setup();
        Ok(())
    }

    pub fn stop_imf(&mut self) -> Result<(), &'static str> {
        self.clear_buffer()?;
        let mut state_ref = RefCell::borrow_mut(&mut self.state);
        let state = state_ref.as_mut().expect("playback init");
        if let Some(imf_state) = state.imf_state.as_mut() {
            imf_state.sq_active = false;
        }
        Ok(())
    }

    pub fn play_adl(&mut self, sound: AdlSound) -> Result<(), &'static str> {
        let mut state_ref = RefCell::borrow_mut(&mut self.state);
        let state = state_ref.as_mut().expect("playback init");

        adl_set_fx_inst(&mut state.chip, &sound.instrument);
        let al_block = ((sound.block & 7) << 2) | 0x20;
        state.adl_state = Some(AdlState {
            sound,
            data_ptr: 0,
            al_block,
            sound_time_counter: state.adl_samples_per_tick,
        });

        Ok(())
    }

    pub fn write_reg(&mut self, reg: u32, val: u8) -> Result<(), &'static str> {
        let mut state_ref = RefCell::borrow_mut(&mut self.state);
        let state = state_ref.as_mut().expect("playback init");

        state.chip.write_reg(reg, val);
        Ok(())
    }

    fn clear_buffer(&self) -> Result<(), &'static str> {
        self.node
            .port()
            .unwrap()
            .post_message(&JsValue::from("CLEAR"))
            .map_err(|_| "failed to send clear command")
    }
}
