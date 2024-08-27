use std::path::Path;

pub mod w3d;

pub struct Track {
    pub no: usize,
    pub name: &'static str,
    pub artist: &'static str,
}

pub struct Metadata {
    pub name: &'static str,
    pub year: usize,
    pub tracks: &'static [Track],
}

type Inferrer = fn() -> bool;
type TrackLoader = fn(game_path: &Path, track_no: usize) -> Result<Vec<u8>, String>;

pub struct GameModule {
    pub game: Game,
    pub metadata: &'static Metadata,
    pub inferrer: Inferrer,
    pub track_loader: TrackLoader,
}

pub enum Game {
    W3D,
    SOD,
}

pub static CATALOGED_GAMES: [&'static GameModule; 1] = [&w3d::GAME_MODULE];

// inferGame() -> Game ?
// each catalog mod contains functions:
// - is_game() -> bool
// - load_track() -> Result<Vec<u8>, String>
// - get_metadata() -> &'static Metadata
