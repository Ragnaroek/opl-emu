use crate::chip::AdlSound;

use js_sys::{Object, Reflect, Uint8Array};
use std::cell::RefCell;
use std::rc::Rc;
use wasm_bindgen::JsValue;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;
use web_sys::{AudioContext, AudioWorkletNode, AudioWorkletNodeOptions};

pub struct OPL {
    audio_ctx: AudioContext,
    node: Option<Rc<AudioWorkletNode>>,

    on_adl_end: Rc<RefCell<Option<Box<dyn FnMut()>>>>,
}

pub struct OPLSettings {
    pub imf_clock_rate: u32,
    pub adl_clock_rate: u32,
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

        Ok(OPL {
            audio_ctx,
            node: None,
            on_adl_end: Rc::new(RefCell::new(None)),
        })
    }

    /// Initialises the background AudioWorklet. This function must be called
    /// from a user-context in a webbrowser. Otherwise the AudioContext init
    /// is denied by the browser.
    pub async fn init(&mut self, settings: OPLSettings) -> Result<(), String> {
        let wasm_bytes = include_bytes!("../web/worklet.wasm");

        let options = AudioWorkletNodeOptions::new();
        options.set_number_of_outputs(1);
        options.set_output_channel_count(&js_sys::Array::of1(&2.into()));

        let processor_options = js_sys::Object::new();
        js_sys::Reflect::set(
            &processor_options,
            &JsValue::from_str("wasmBytes"),
            &js_sys::Uint8Array::from(wasm_bytes.as_slice()),
        )
        .map_err(|_| "err setting wasm bytes")?;
        js_sys::Reflect::set(
            &processor_options,
            &JsValue::from_str("mixerRate"),
            &(self.audio_ctx.sample_rate() as u32).into(),
        )
        .map_err(|_| "err setting mixerRate")?;
        js_sys::Reflect::set(
            &processor_options,
            &JsValue::from_str("imfClockRate"),
            &settings.imf_clock_rate.into(),
        )
        .map_err(|_| "err setting imfClockRate")?;
        js_sys::Reflect::set(
            &processor_options,
            &JsValue::from_str("adlClockRate"),
            &settings.adl_clock_rate.into(),
        )
        .map_err(|_| "err setting adlClockRate")?;

        options.set_processor_options(Some(&processor_options.into()));

        let node = AudioWorkletNode::new_with_options(&self.audio_ctx, "opl-processor", &options)
            .map_err(|_| "err creating AudioWorkletNode")?;

        let on_adl_end_clone = self.on_adl_end.clone();
        let on_message = Closure::<dyn FnMut(web_sys::MessageEvent)>::new(
            move |event: web_sys::MessageEvent| {
                let data = event.data();
                if let Some(cmd) = data.as_string() {
                    if cmd == "adl_finished" {
                        if let Some(mut cb) = on_adl_end_clone.borrow_mut().take() {
                            cb()
                        }
                    }
                }
            },
        );
        node.port()
            .unwrap()
            .set_onmessage(Some(on_message.as_ref().unchecked_ref()));
        on_message.forget();

        node.connect_with_audio_node(&self.audio_ctx.destination())
            .map_err(|_| "err connecting with audio node")?;

        JsFuture::from(self.audio_ctx.resume().map_err(|_| "resume failed")?)
            .await
            .map_err(|_| "failed to resume audio context")?;

        self.node = Some(Rc::new(node));

        Ok(())
    }

    pub fn play_imf(&mut self, data: Vec<u8>) -> Result<(), &'static str> {
        self.send_data_cmd("play_imf", data)
    }

    pub fn play_adl<F>(&mut self, sound: AdlSound, on_end: F) -> Result<(), &'static str>
    where
        F: FnMut() + 'static,
    {
        *self.on_adl_end.borrow_mut() = Some(Box::new(on_end));
        let data = sound.to_vec();
        self.send_data_cmd("play_adl", data)
    }

    pub fn write_reg(&mut self, reg: u32, val: u8) -> Result<(), &'static str> {
        let cmd = cmd_object("write_reg")?;
        Reflect::set(&cmd, &"reg".into(), &reg.into()).map_err(|_| "err setting reg")?;
        Reflect::set(&cmd, &"value".into(), &val.into()).map_err(|_| "err setting value")?;
        self.send_cmd(cmd)
    }

    pub fn stop_imf(&mut self) -> Result<(), &'static str> {
        let cmd = cmd_object("stop_imf")?;
        self.send_cmd(cmd)
    }

    fn send_data_cmd(&mut self, cmd_name: &'static str, data: Vec<u8>) -> Result<(), &'static str> {
        let cmd = cmd_object(cmd_name)?;
        let js_data = Uint8Array::from(&data[..]);
        Reflect::set(&cmd, &"data".into(), &js_data).map_err(|_| "err setting data")?;
        self.send_cmd(cmd)
    }

    fn send_cmd(&mut self, cmd: Object) -> Result<(), &'static str> {
        if let Some(node) = &self.node {
            node.port()
                .unwrap()
                .post_message(&cmd.into())
                .map_err(|_| "err sending command")
        } else {
            Err("OPL not initialised")
        }
    }
}

fn cmd_object(cmd_name: &'static str) -> Result<Object, &'static str> {
    let cmd = Object::new();
    Reflect::set(&cmd, &"cmd".into(), &cmd_name.into()).map_err(|_| "err setting cmd")?;
    Ok(cmd)
}
