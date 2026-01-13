use std::env;
use std::fs;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use opl::{OPL, OPLSettings};
use sdl2::audio::{self, AudioCVT, AudioFormat};
use sdl2::mixer::{self};

const ORIG_SAMPLE_RATE: i32 = 7042;

fn main() -> Result<(), String> {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: {} <track-filename> [<sound-filename>]", args[0]);
        std::process::exit(1);
    }

    let track_file_name = &args[1];
    let track_file_data = fs::read(track_file_name).expect("Failed to read track file");

    let mut adl: Option<opl::chip::AdlSound> = None;
    let mut digi_raw: Option<Vec<u8>> = None;
    if args.len() >= 3 {
        let file = &args[2];
        let sound_file_data = fs::read(file).expect("Failed to read sound file");
        if file.ends_with(".adl") {
            adl = Some(opl::chip::AdlSound::from_bytes(&sound_file_data));
        } else if file.ends_with(".digi") {
            digi_raw = Some(sound_file_data)
        } else {
            return Err(format!("unknown file type: {}", file));
        }
    }

    let mut opl = OPL::new()?;
    opl.init(OPLSettings {
        mixer_rate: 44100,
        imf_clock_rate: 560,
        adl_clock_rate: 140,
    });

    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();
    ctrlc::set_handler(move || {
        r.store(false, Ordering::SeqCst);
    })
    .map_err(|e| e.to_string())?;

    mixer::open_audio(44100, mixer::AUDIO_S16LSB, 2, 2048)?;
    let (mix_freq, mix_format, mix_channels) = mixer::query_spec()?;
    mixer::reserve_channels(2);
    let group = mixer::Group(1);
    group.add_channels_range(2, 8 - 1);

    let mix_config = DigiMixConfig {
        frequency: mix_freq,
        format: map_audio_format(mix_format),
        channels: mix_channels,
        group,
    };

    let cvt = AudioCVT::new(
        audio::AudioFormat::U8,
        1,
        ORIG_SAMPLE_RATE,
        mix_config.format,
        mix_config.channels as u8,
        mix_config.frequency,
    )?;

    let digi = if let Some(digi_raw_bytes) = digi_raw {
        let converted_data = cvt.convert(digi_raw_bytes);
        let chunk =
            mixer::Chunk::from_raw_buffer(converted_data.into_boxed_slice()).expect("chunk");
        Some(chunk)
    } else {
        None
    };

    let mixer_channel = mix_config
        .group
        .find_available()
        .expect("get sdl mixer channel");

    let mut digi_started = false;

    opl.play_imf(track_file_data)?;
    println!("Playing track... Press Ctrl+C to stop");
    while running.load(Ordering::SeqCst) {
        if !opl.is_imf_playing()? {
            println!("Track finished");
            break;
        }
        if adl.is_some() && !opl.is_adl_playing()? {
            let adl_play = adl.as_ref().expect("sound file").clone();
            opl.play_adl(adl_play)?;
        }

        if let Some(digi_chunk) = &digi
            && !digi_started
        {
            mixer_channel.halt();
            mixer_channel.play(digi_chunk, -1)?;
            digi_started = true;
        }

        std::thread::sleep(Duration::from_millis(50));
    }

    Ok(())
}

pub struct DigiMixConfig {
    pub frequency: i32,
    pub format: AudioFormat,
    pub channels: i32,
    pub group: mixer::Group,
}

fn map_audio_format(format: mixer::AudioFormat) -> AudioFormat {
    match format {
        mixer::AUDIO_S16LSB => AudioFormat::S16LSB,
        _ => todo!("impl mapping"),
    }
}
