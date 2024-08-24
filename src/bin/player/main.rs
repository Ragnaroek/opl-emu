use clap::Parser;
use opl::file_db_w3d::{read_music_track, read_w3d_header};
use std::{
    env,
    fs::File,
    io::Read,
    os::unix::fs::FileExt,
    path::{Path, PathBuf},
    str::FromStr,
};

#[derive(Parser)]
struct Cli {
    /// Path to the folder that contains the game files or
    /// a OPL file to play. If no path is supplied the cwd is taken.
    path: Option<std::path::PathBuf>,
}

pub fn main() -> Result<(), String> {
    let args = Cli::parse();

    let path = if let Some(path) = args.path {
        path
    } else {
        env::current_dir().map_err(|e| e.to_string())?
    };

    let track_data = if path.is_dir() {
        let headers = read_w3d_header(&path.join("AUDIOHED.WL6"))?;
        read_music_track(&headers, &path.join("AUDIOT.WL6"), 0)?
    } else {
        read_file(&path)
    };

    let mut opl = opl::new().expect("opl setup");
    let settings = opl::OPLSettings {
        mixer_rate: 49716,
        imf_clock_rate: 0,
    };
    opl.play(track_data, settings).expect("play");

    let mut line = String::new();
    let _ = std::io::stdin()
        .read_line(&mut line)
        .expect("Failed to read line");
    Ok(())
}

// Assumes a 'ripped AudioT chunk' as for now
fn read_file(file: &Path) -> Vec<u8> {
    let mut file = File::open(file).expect("open audio file");
    let mut size_buf: [u8; 2] = [0; 2];
    let bytes_read = file.read(&mut size_buf).expect("read size");
    if bytes_read != 2 {
        panic!("invalid file {:?}, could not read size header", file);
    }

    let size = u16::from_le_bytes(size_buf) as usize;

    let mut bytes = vec![0; size];
    file.read_exact_at(&mut bytes, 2).expect("read data");
    bytes
}
