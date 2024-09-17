use clap::Parser;
use opl::{
    catalog::w3d::{load_chunk, load_track, read_w3d_header, AUDIO_FILE, HEADER_FILE},
    AdlSound, Instrument,
};
use ratatui::crossterm::{
    event::{self, Event, KeyCode, KeyEventKind},
    terminal::{disable_raw_mode, enable_raw_mode},
};
use std::{
    env,
    io::{self, Read},
    str,
};

#[derive(Parser)]
struct Cli {
    /// Path to the folder that contains the game files. If non
    /// is supplied the cwd is taken.
    #[arg(short, long)]
    folder: Option<std::path::PathBuf>,
    /// sound chunk no to play in a loop
    sound_no: usize,
}

// Test program, mainly created to test the mixing of OPL music track playing
// with ADL sound effects.
pub fn main() -> Result<(), String> {
    let args = Cli::parse();

    let folder_path = if let Some(path) = args.folder {
        path
    } else {
        env::current_dir().map_err(|e| e.to_string())?
    };

    let mut sound_no = args.sound_no;

    let music_track = load_track(&folder_path, 0).expect("music track");
    let headers = read_w3d_header(&folder_path.join(HEADER_FILE))?;

    let mut opl = opl::new()?;
    // set up the OPL with frequencies to play W3D sounds
    let settings = opl::OPLSettings {
        mixer_rate: 49716,
        imf_clock_rate: 700,
        adl_clock_rate: 140,
    };
    opl.init(settings);

    opl.play_imf(music_track)?;

    enable_raw_mode().map_err(|e| e.to_string())?;
    println!("Press 'q' to quit, 's' to play a sound or 'c'/'t' to choose a sound/track");
    loop {
        let evt = event::read().map_err(|e| e.to_string())?;
        if let Event::Key(key) = evt {
            if key.kind == KeyEventKind::Press {
                match key.code {
                    KeyCode::Char('q') => break,
                    KeyCode::Char('s') => {
                        let chunk =
                            load_chunk(&headers, &folder_path.join(AUDIO_FILE), sound_no, 0)?;
                        let sound = read_sound(chunk);
                        opl.play_adl(sound)?
                    }
                    KeyCode::Char('c') => {
                        todo!("sound choosing");
                    }
                    KeyCode::Char('t') => {
                        todo!("track choosing");
                    }
                    _ => { /* ignore */ }
                }
            }
        }
    }
    disable_raw_mode().map_err(|e| e.to_string())?;
    Ok(())
}

fn read_sound(data: Vec<u8>) -> AdlSound {
    let length = u32::from_le_bytes(data[0..4].try_into().unwrap());
    let instrument = Instrument {
        m_char: data[6],
        c_char: data[7],
        m_scale: data[8],
        c_scale: data[9],
        m_attack: data[10],
        c_attack: data[11],
        m_sus: data[12],
        c_sus: data[13],
        m_wave: data[14],
        c_wave: data[15],
        n_conn: data[16],
        voice: data[17],
        mode: data[18],
        // data[19..22] are padding and omitted
    };
    AdlSound {
        length,
        priority: u16::from_le_bytes(data[4..6].try_into().unwrap()),
        instrument,
        block: data[22],
        data: data[23..(23 + length as usize)].to_vec(),
        terminator: data[23 + length as usize],
        name: str::from_utf8(&data[(23 + length as usize) + 1..data.len() - 1])
            .expect("sound name")
            .to_string(),
    }
}
