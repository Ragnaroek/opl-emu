use clap::{CommandFactory, Parser};
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

    /// Track number to extract (either track_no or sound_no or digi_no have to be supplied)
    #[arg(long)]
    track_no: Option<usize>,

    /// Sound number to extract (either track_no or sound_no or digi_no have to be supplied)
    #[arg(long)]
    sound_no: Option<usize>,

    /// Digital sound number to extract (either track_no or sound_no or digi_no have to be supplied)
    #[arg(long)]
    digi_no: Option<usize>,
}

// TODO generalize to other formats
pub fn main() -> Result<(), String> {
    let args = Cli::parse();

    let folder_path = if let Some(path) = args.folder {
        path
    } else {
        env::current_dir().map_err(|e| e.to_string())?
    };

    if let Some(track_no) = args.track_no {
        extract_track(&folder_path, track_no)?;
    } else if let Some(sound_no) = args.sound_no {
        extract_sound(&folder_path, sound_no)?;
    } else if let Some(digi_no) = args.digi_no {
        extract_digi(&folder_path, digi_no)?;
    } else {
        let mut cmd = Cli::command();
        cmd.print_help().map_err(|e| e.to_string())?;
        std::process::exit(0);
    }

    Ok(())
}

fn extract_sound(folder_path: &Path, sound_no: usize) -> Result<(), String> {
    let sound_data = w3d::load_sound(&folder_path, sound_no)?;

    let file_name = &format!("sound_{}.adl", sound_no);
    let mut file = File::create(Path::new(file_name)).map_err(|e| e.to_string())?;
    file.write_all(&sound_data).map_err(|e| e.to_string())?;
    println!("file {} written", file_name);
    Ok(())
}

fn extract_track(folder_path: &Path, track_no: usize) -> Result<(), String> {
    if track_no >= w3d::GAME_MODULE.metadata.tracks.len() {
        return Err(format!("track number {} is out of range", track_no));
    }

    let track_meta = &w3d::GAME_MODULE.metadata.tracks[track_no];
    println!(
        "extracting track: {} - {}",
        track_meta.name, track_meta.artist
    );

    let track_data = w3d::load_track(&folder_path, track_no)?;

    let file_name = &format!("track_{}.imf", track_no);
    let mut file = File::create(Path::new(file_name)).map_err(|e| e.to_string())?;
    file.write_all(&track_data).map_err(|e| e.to_string())?;
    println!("file {} written", file_name);
    Ok(())
}

fn extract_digi(folder_path: &Path, digi_no: usize) -> Result<(), String> {
    let digi_data = w3d::load_digi(&folder_path, digi_no)?;

    let file_name = &format!("sound_{}.digi", digi_no);
    let mut file = File::create(Path::new(file_name)).map_err(|e| e.to_string())?;
    file.write_all(&digi_data).map_err(|e| e.to_string())?;
    println!("file {} written", file_name);
    Ok(())
}
