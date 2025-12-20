use std::env;
use std::fs;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

fn main() -> Result<(), String> {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: {} <filename>", args[0]);
        std::process::exit(1);
    }

    let track_data = fs::read(&args[1]).expect("Failed to read file");

    let mut opl = opl::new()?;
    opl.init(opl::OPLSettings {
        mixer_rate: 49716,
        imf_clock_rate: 0,
        adl_clock_rate: 0,
    });

    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();
    ctrlc::set_handler(move || {
        r.store(false, Ordering::SeqCst);
    })
    .map_err(|e| e.to_string())?;

    opl.play_imf(track_data)?;

    println!("Playing track... Press Ctrl+C to stop");
    while running.load(Ordering::SeqCst) {
        if !opl.is_imf_playing()? {
            println!("Track finished");
            break;
        }
        std::thread::sleep_ms(50);
    }

    Ok(())
}
