use clap::Parser;
use opl::catalog::w3d::{read_music_track, read_w3d_header};
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

pub fn main() -> Result<(), String> {
    let args = Cli::parse();

    let folder_path = if let Some(path) = args.folder {
        path
    } else {
        env::current_dir().map_err(|e| e.to_string())?
    };

    let headers = read_w3d_header(&folder_path.join("AUDIOHED.WL6"))?;
    let track_data = read_music_track(&headers, &folder_path.join("AUDIOT.WL6"), args.track_no)?;

    std::io::stdout()
        .write_all(&track_data)
        .map_err(|e| e.to_string())?;
    std::io::stdout().flush().map_err(|e| e.to_string())?;

    Ok(())
}
