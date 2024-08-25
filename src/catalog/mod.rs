pub mod w3d;

pub struct Track {
    pub name: String,
}

pub struct Metadata {
    pub name: String,
    pub tracks: Vec<Track>,
}

pub enum Game {
    W3D,
    SOD,
}

// inferGame() -> Game ?
// each catalog mod contains functions:
// - is_game() -> bool
// - load_track() -> Result<Vec<u8>, String>
// - get_metadata() -> &'static Metadata
