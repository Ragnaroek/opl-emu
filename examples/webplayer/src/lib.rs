use wasm_bindgen::prelude::*;
use web_sys::{AudioContext, AudioContextOptions};

use opl::{OPL, OPLSettings, chip::AdlSound};

const DIGI_SAMPLE_RATE: f32 = 7042.0;

#[wasm_bindgen]
pub struct WebControl {
    opl: OPL,
    digi_context: AudioContext,
}

#[wasm_bindgen]
pub async fn init_player() -> Result<WebControl, String> {
    console_error_panic_hook::set_once();

    let mut opl = OPL::new().await?;
    opl.init(OPLSettings {
        imf_clock_rate: 560,
        adl_clock_rate: 140,
    })
    .await?;

    let digi_context = init_digi_sound_context()?;

    Ok(WebControl { opl, digi_context })
}

#[wasm_bindgen]
impl WebControl {
    pub async fn play_imf(&mut self, track_data: Vec<u8>) {
        self.opl.stop_imf().expect("stop_imf");
        self.opl.play_imf(track_data).expect("play imf")
    }

    pub async fn play_adl(&mut self, sound_data: Vec<u8>) {
        let adl = AdlSound::from_bytes(&sound_data);
        self.opl.play_adl(adl).expect("play adl");
    }

    pub async fn play_digi(&mut self, digi_data: Vec<u8>) {
        let converted: Vec<f32> = digi_data
            .iter()
            .map(|&s| (s as f32 - 128.0) / 128.0)
            .collect();

        let frames = converted.len() as u32;
        let buffer = self
            .digi_context
            .create_buffer(1, frames, DIGI_SAMPLE_RATE)
            .expect("buffer");

        buffer
            .copy_to_channel_with_start_in_channel(&converted, 0, 0)
            .expect("copied data to channel");

        let src = self
            .digi_context
            .create_buffer_source()
            .expect("buffer source creation");
        src.set_buffer(Some(&buffer));
        src.set_loop(true);

        src.connect_with_audio_node(&self.digi_context.destination())
            .expect("audio connect");

        src.start().expect("sound start")
    }
}

fn init_digi_sound_context() -> Result<AudioContext, String> {
    let opts = AudioContextOptions::new();
    opts.set_sample_rate(44100.0);

    let ctx =
        AudioContext::new_with_context_options(&opts).map_err(|_| "digi audio context init")?;
    Ok(ctx)
}
