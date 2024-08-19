use sdl2::audio::{AudioCallback, AudioDevice, AudioSpecDesired};
use sdl2::{self, AudioSubsystem};

use crate::{Chip, OPLSettings};

pub struct OPL {
    audio_subsystem: AudioSubsystem,
    device: Option<AudioDevice<OPLCallback>>,
}

pub fn new() -> Result<OPL, &'static str> {
    let sdl_context = sdl2::init().expect("sdl init failed");
    let audio_subsystem = sdl_context.audio().expect("audio init failed");
    Ok(OPL {
        audio_subsystem,
        device: None,
    })
}

impl OPL {
    pub fn play(&mut self, data: Vec<u8>, settings: OPLSettings) -> Result<(), &'static str> {
        self.ensure_device(settings);

        let device = self.device.as_mut().expect("device");
        {
            let mut cb = device.lock();
            cb.hack_len = data.len();
            cb.hack_seq_len = data.len();
            cb.hack_time = 0;
            cb.al_time_count = 0;
            cb.hack_ptr = 0;
            cb.data = data;

            cb.chip.setup();
        }

        device.resume();
        Ok(())
    }

    fn ensure_device(&mut self, settings: OPLSettings) {
        if self.device.is_none() {
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

            let device = self
                .audio_subsystem
                .open_playback(None, &desired_spec, |_| {
                    // initialize the audio callback
                    OPLCallback {
                        mix_buffer: vec![0; settings.mixer_rate as usize],
                        num_ready_samples: 0,
                        samples_per_music_tick,
                        data: Vec::new(),
                        chip: Chip::new(settings.mixer_rate),
                        hack_len: 0,
                        hack_seq_len: 0,
                        hack_ptr: 0,
                        hack_time: 0,
                        al_time_count: 0,
                    }
                })
                .expect("playback open failed");
            /*
            println!(
                "## samples = {:?}, size={}",
                desired_spec.samples,
                device.spec().size
            );*/

            self.device = Some(device);
        } else {
            println!("### device set");
        }
    }
}

struct OPLCallback {
    mix_buffer: Vec<i32>,
    num_ready_samples: u32,
    samples_per_music_tick: u32,

    data: Vec<u8>,
    chip: Chip,

    //sequencer variables
    hack_ptr: usize,
    hack_len: usize,
    hack_seq_len: usize,
    hack_time: u32,
    al_time_count: u32,
}

impl AudioCallback for OPLCallback {
    type Channel = i16;

    fn callback(&mut self, out: &mut [i16]) {
        let mut samples_len = out.len() as u32 >> 1;
        println!(
            "### callback, len = {}, sampleslen = {}",
            out.len(),
            samples_len
        );
        let mut out_offset = 0;

        loop {
            println!("num_ready_samples = {}", self.num_ready_samples);
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
                    return; //wait for next callback
                }
            }

            loop {
                if self.hack_time > self.al_time_count {
                    break;
                }

                let t = u16::from_le_bytes(
                    self.data[(self.hack_ptr + 2)..(self.hack_ptr + 4)]
                        .try_into()
                        .unwrap(),
                ) as u32;
                self.hack_time = self.al_time_count + t;

                let reg = self.data[self.hack_ptr] as u32;
                let val = self.data[self.hack_ptr + 1];

                self.chip.write_reg(reg, val);
                self.hack_ptr += 4;
                self.hack_len -= 4;

                if self.hack_len <= 0 {
                    break;
                }
            }
            self.al_time_count += 1;
            if self.hack_len == 0 {
                self.hack_ptr = 0;
                self.hack_len = self.hack_seq_len;
                self.hack_time = 0;
                self.al_time_count = 0;
            }

            self.num_ready_samples = self.samples_per_music_tick;
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
    //println!("\n\ncall_count = {}", chip.call_count);
    let debug_count = 30;

    chip.generate_block_2(len, mix_buffer);

    let mut mix_ptr = 0;
    let mut out_ptr = offset;
    for _ in 0..len {
        let mix = (mix_buffer[mix_ptr] << 2) as i16; // increase volume a bit

        /*
        if chip.call_count == debug_count {
            println!("mix = {}", mix);
        }

        if mix != 0 {
            println!("mix = {}", mix);
            panic!("exit");
        }*/

        mix_ptr += 1;

        sdl_out[out_ptr] = mix;
        out_ptr += 1;
        sdl_out[out_ptr] = mix;
        out_ptr += 1;
    }

    if chip.call_count == debug_count {
        panic!("exit");
    }

    chip.call_count += 1;
}
