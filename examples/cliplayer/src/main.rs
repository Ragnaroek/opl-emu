use std::env;
use std::fs;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use opl::{OPL, OPLSettings};

fn main() -> Result<(), String> {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: {} <track-filename> [<sound-filename>]", args[0]);
        std::process::exit(1);
    }

    let track_file_name = &args[1];
    let track_file_data = fs::read(track_file_name).expect("Failed to read track file");

    let mut adl: Option<opl::chip::AdlSound> = None;
    if args.len() >= 3 {
        let sound_file_data = fs::read(&args[2]).expect("Failed to read sound file");
        adl = Some(opl::chip::read_adl(&sound_file_data));
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
        std::thread::sleep(Duration::from_millis(50));
    }

    Ok(())
}
