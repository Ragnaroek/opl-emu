use wasm_bindgen::prelude::*;

use opl::{OPL, OPLSettings};

#[wasm_bindgen]
pub async fn start_player(track_data: Vec<u8>) -> Result<(), String> {
    console_error_panic_hook::set_once();

    let mut opl = OPL::new().await?;
    opl.init(OPLSettings {
        mixer_rate: 44100,
        imf_clock_rate: 560,
        adl_clock_rate: 140,
    })
    .await?;

    opl.play_imf(track_data)?;
    Ok(())
}

pub async fn sleep(millis: u32) {
    let mut cb = |resolve: js_sys::Function, _reject: js_sys::Function| {
        let win = web_sys::window().expect("web_sys window");
        win.set_timeout_with_callback_and_timeout_and_arguments_0(&resolve, millis as i32)
            .expect("timeout set");
    };
    let p = js_sys::Promise::new(&mut cb);
    wasm_bindgen_futures::JsFuture::from(p).await.unwrap();
}
