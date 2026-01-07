extern crate alloc;

use crate::chip::{AL_FREQ_H, AL_FREQ_L, AdlSound, Chip, adl_set_fx_inst};
use alloc::boxed::Box;
use alloc::vec;
use alloc::vec::Vec;
use core::panic::PanicInfo;
use core::slice;

use mini_alloc::MiniAlloc;

#[global_allocator]
static ALLOC: MiniAlloc = MiniAlloc::INIT;

const BLOCK_LEN: usize = 256;
const FLOAT_CONVERSION_FACTOR: f32 = 32768.0;

pub struct WorkletImfState {
    pub data_ptr: *const u8,
    pub data_len: usize,

    pub hack_ptr: usize,
    pub hack_len: usize,
    pub hack_seq_len: usize,
    pub hack_time: u32,
    pub al_time_count: u32,
    pub sq_active: bool,
}

pub struct WorkletAdlState {
    pub sound: AdlSound,
    pub data_ptr: usize,
    pub sound_time_counter: u32,
    pub al_block: u8,
}

#[repr(C)]
pub struct OplGenerator {
    mix_buffer: Vec<i32>,
    buf: [f32; BLOCK_LEN],
    chip: Chip,
    imf_state: Option<WorkletImfState>,
    adl_state: Option<WorkletAdlState>,
    num_ready_samples: u32,
    samples_per_music_tick: u32,
    adl_samples_per_tick: u32,
}

#[unsafe(no_mangle)]
pub extern "C" fn alloc(size: usize) -> *mut u8 {
    let mut buf = Vec::<u8>::with_capacity(size);
    let ptr = buf.as_mut_ptr();
    core::mem::forget(buf);
    ptr
}

#[unsafe(no_mangle)]
pub extern "C" fn dealloc(ptr: *mut u8, size: usize) {
    unsafe {
        let _ = Vec::from_raw_parts(ptr, size, size);
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn new_generator(
    mixer_rate: u32,
    imf_clock_rate_param: u32,
    adl_clock_rate_param: u32,
) -> *mut OplGenerator {
    let chip = Chip::new(mixer_rate);

    let imf_clock_rate = if imf_clock_rate_param == 0 {
        700
    } else {
        imf_clock_rate_param
    };
    let adl_clock_rate = if adl_clock_rate_param == 0 {
        140
    } else {
        adl_clock_rate_param
    };
    let samples_per_music_tick = mixer_rate / imf_clock_rate;
    let adl_samples_per_tick = imf_clock_rate / adl_clock_rate;

    let mix_buffer = vec![0; samples_per_music_tick as usize];

    Box::into_raw(Box::new(OplGenerator {
        mix_buffer,
        buf: [0.0; BLOCK_LEN],
        chip,
        imf_state: None,
        adl_state: None,
        num_ready_samples: 0,
        samples_per_music_tick,
        adl_samples_per_tick,
    }))
}

#[unsafe(no_mangle)]
pub extern "C" fn generate_block(g: *mut OplGenerator) -> *const f32 {
    let mut samples_len = (BLOCK_LEN >> 1) as u32;
    let mut out_offset = 0;

    unsafe {
        if (*g).imf_state.is_some() {
            loop {
                if (*g).num_ready_samples > 0 {
                    if (*g).num_ready_samples < samples_len {
                        opl_update(g, out_offset, (*g).num_ready_samples as usize);
                        out_offset += (*g).num_ready_samples * 2;
                        samples_len -= (*g).num_ready_samples;
                    } else {
                        opl_update(g, out_offset, samples_len as usize);
                        (*g).num_ready_samples -= samples_len;
                        break;
                    }
                }

                if (*g).adl_state.is_some() {
                    let state = (*g).adl_state.as_mut().expect("adl state");
                    state.sound_time_counter -= 1;
                    if state.sound_time_counter == 0 {
                        state.sound_time_counter = (*g).adl_samples_per_tick;
                        if state.data_ptr < state.sound.data.len() {
                            let al_sound = state.sound.data[state.data_ptr];
                            if al_sound != 0 {
                                (*g).chip.write_reg(AL_FREQ_L, al_sound);
                                (*g).chip.write_reg(AL_FREQ_H, state.al_block);
                            } else {
                                (*g).chip.write_reg(AL_FREQ_H, 0);
                            }
                            state.data_ptr += 1;
                        } else {
                            (*g).adl_state = None;
                            (*g).chip.write_reg(AL_FREQ_H, 0); // write silence at the end so that last note does not repeat
                        }
                    }
                }

                let imf_state = (*g).imf_state.as_mut().unwrap();
                if imf_state.sq_active {
                    loop {
                        if imf_state.hack_time > imf_state.al_time_count {
                            break;
                        }

                        let imf_data =
                            core::slice::from_raw_parts(imf_state.data_ptr, imf_state.data_len);

                        let bytes = imf_data[(imf_state.hack_ptr + 2)..(imf_state.hack_ptr + 4)]
                            .try_into()
                            .unwrap();
                        let t = u16::from_le_bytes(bytes) as u32;

                        imf_state.hack_time = imf_state.al_time_count + t;

                        let reg = imf_data[imf_state.hack_ptr] as u32;
                        let val = imf_data[imf_state.hack_ptr + 1];

                        (*g).chip.write_reg(reg, val);
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

                (*g).num_ready_samples = (*g).samples_per_music_tick;
            }
        }
        (*g).buf.as_ptr()
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn play_imf(g: *mut OplGenerator, ptr: *const u8, len: usize) {
    unsafe {
        (*g).imf_state = Some(WorkletImfState {
            data_ptr: ptr,
            data_len: len,
            hack_len: len,
            hack_seq_len: len,
            hack_time: 0,
            al_time_count: 0,
            hack_ptr: 0,
            sq_active: true,
        });
        (*g).chip.setup();
    };
}

#[unsafe(no_mangle)]
pub extern "C" fn stop_imf(g: *mut OplGenerator) {
    unsafe {
        if let Some(imf_state) = &mut (*g).imf_state {
            imf_state.sq_active = false;
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn play_adl(g: *mut OplGenerator, ptr: *const u8, len: usize) {
    unsafe {
        let data = slice::from_raw_parts(ptr, len);
        let sound = AdlSound::from_bytes(data);
        adl_set_fx_inst(&mut (*g).chip, &sound.instrument);
        let al_block = ((sound.block & 7) << 2) | 0x20;
        (*g).adl_state = Some(WorkletAdlState {
            sound,
            data_ptr: 0,
            al_block,
            sound_time_counter: (*g).adl_samples_per_tick,
        });
    };
}

#[unsafe(no_mangle)]
pub extern "C" fn write_reg(g: *mut OplGenerator, reg: u32, val: u8) {
    unsafe { (*g).chip.write_reg(reg, val) }
}

fn opl_update(g: *mut OplGenerator, offset: u32, len: usize) {
    unsafe {
        (*g).chip.generate_block_2(len, &mut (*g).mix_buffer);

        let mut mix_ptr = 0;
        let mut out_ptr = offset as usize;
        for _ in 0..len {
            let mix = ((&(*g).mix_buffer)[mix_ptr] << 2) as f32; // increase volume a bit
            mix_ptr += 1;

            let f32_mix_val = mix / FLOAT_CONVERSION_FACTOR;
            (*g).buf[out_ptr] = f32_mix_val;
            out_ptr += 1;
            (*g).buf[out_ptr] = f32_mix_val;
            out_ptr += 1;
        }
    }
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    core::arch::wasm32::unreachable()
}
