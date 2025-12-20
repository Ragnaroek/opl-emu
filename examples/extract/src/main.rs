use clap::Parser;
use opl::catalog::w3d;
use std::fs::File;
use std::path::Path;
use std::{env, io::Write};

#[derive(Parser)]
struct Cli {
    /// Path to the folder that contains the game files. If non
    /// is supplied the cwd is taken.
    #[arg(short, long)]
    folder: Option<std::path::PathBuf>,
    /// Track number to extract
    track_no: usize,
}

// TODO generalize to other formats
pub fn main() -> Result<(), String> {
    let args = Cli::parse();

    let folder_path = if let Some(path) = args.folder {
        path
    } else {
        env::current_dir().map_err(|e| e.to_string())?
    };

    if args.track_no >= w3d::GAME_MODULE.metadata.tracks.len() {
        return Err(format!("track number {} is out of range", args.track_no));
    }

    let track_meta = &w3d::GAME_MODULE.metadata.tracks[args.track_no];
    println!(
        "extracting track: {} - {}",
        track_meta.name, track_meta.artist
    );

    let track_data = w3d::load_track(&folder_path, args.track_no)?;

    let file_name = &format!("track_{}.imf", args.track_no);
    let mut file = File::create(Path::new(file_name)).map_err(|e| e.to_string())?;
    file.write_all(&track_data).map_err(|e| e.to_string())?;
    println!("file {} written", file_name);

    Ok(())
}
