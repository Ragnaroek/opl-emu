use std::{fs::File, io::Read, os::unix::fs::FileExt, path::Path};

use super::{GameModule, Metadata, Track};

pub static GAME_MODULE: GameModule = GameModule {
    game: super::Game::W3D,
    metadata: &METADATA,
    inferrer: is_w3d,
    track_loader: load_track,
};

static METADATA: Metadata = Metadata {
    name: "Wolfenstein 3D",
    year: 1992,
    tracks: &[
        Track {
            no: 0,
            artist: "Bobby Prince",
            name: "Enemy Around the Corner",
        },
        Track {
            no: 1,
            artist: "Bobby Prince",
            name: "Into the Dungeons",
        },
        Track {
            no: 2,
            artist: "Bobby Prince",
            name: "March to War",
        },
        Track {
            no: 3,
            artist: "Bobby Prince",
            name: "Get Them Before They Get You",
        },
        Track {
            no: 4,
            artist: "Bobby Prince",
            name: "Pounding Headache",
        },
        Track {
            no: 5,
            artist: "Bobby Prince",
            name: "The Hitler Waltz",
        },
        Track {
            no: 6,
            artist: "Bobby Prince",
            name: "Kill the S.O.B.",
        },
        Track {
            no: 7,
            artist: "Bobby Prince",
            name: "Horst-Wessel-Lied (Nazi Anthem)",
        },
        Track {
            no: 8,
            artist: "Bobby Prince",
            name: "Nazi Anthem",
        },
        Track {
            no: 9,
            artist: "Bobby Prince",
            name: "Prisoner of War",
        },
        Track {
            no: 10,
            artist: "Bobby Prince",
            name: "Salutation",
        },
        Track {
            no: 11,
            artist: "Bobby Prince",
            name: "Searching for the Enemy",
        },
        Track {
            no: 12,
            artist: "Bobby Prince",
            name: "Suspense",
        },
        Track {
            no: 13,
            artist: "Bobby Prince",
            name: "Victor's Funeral",
        },
        Track {
            no: 14,
            artist: "Bobby Prince",
            name: "Wondering About My Loved Ones",
        },
        Track {
            no: 15,
            artist: "Bobby Prince",
            name: "Funk You!",
        },
        Track {
            no: 16,
            artist: "Bobby Prince",
            name: "Intermission Song",
        },
        Track {
            no: 17,
            artist: "Bobby Prince",
            name: "Going After Hitler",
        },
        Track {
            no: 18,
            artist: "Bobby Prince",
            name: "Lurking",
        },
        Track {
            no: 19,
            artist: "Bobby Prince",
            name: "The Ultimate Challenge",
        },
        Track {
            no: 20,
            artist: "Bobby Prince",
            name: "The Nazi Rap",
        },
        Track {
            no: 21,
            artist: "Bobby Prince",
            name: "Zero Hour",
        },
        Track {
            no: 22,
            artist: "Bobby Prince",
            name: "The Twelfth Hour",
        },
        Track {
            no: 23,
            artist: "Bobby Prince",
            name: "High Scores Music",
        },
        Track {
            no: 24,
            artist: "Bobby Prince",
            name: "Episode Ending Music",
        },
        Track {
            no: 25,
            artist: "Bobby Prince",
            name: "March of the Victorians",
        },
        Track {
            no: 26,
            artist: "Bobby Prince",
            name: "Pac-Man",
        },
    ],
};

pub static HEADER_FILE: &str = "AUDIOHED.WL6";
pub static AUDIO_FILE: &str = "AUDIOT.WL6";

pub const START_SOUND: usize = 87;
pub const START_MUSIC: usize = 261;

// Game Module interface
fn is_w3d() -> bool {
    todo!("check w3d folder structure");
}

pub fn load_track(game_path: &Path, track_no: usize) -> Result<Vec<u8>, String> {
    let headers = read_w3d_header(&game_path.join(HEADER_FILE))?;
    load_chunk(
        &headers,
        &game_path.join(AUDIO_FILE),
        START_MUSIC + track_no,
        2,
    )
}
// End Game Module interface

// Reads a raw chunk from the W3D audiofile.
pub fn load_chunk(
    headers: &Vec<u32>,
    audiot_file: &Path,
    chunk_no: usize,
    data_start: usize,
) -> Result<Vec<u8>, String> {
    let file = File::open(audiot_file).map_err(|e| e.to_string())?;
    let offset = headers[chunk_no];
    let size = (headers[chunk_no + 1] - offset) as usize;

    let mut data_buf = vec![0; size];
    file.read_exact_at(&mut data_buf, offset as u64)
        .map_err(|e| e.to_string())?;

    let track_size = u16::from_le_bytes(data_buf[0..2].try_into().unwrap()) as usize;
    let mut result = vec![0; track_size];
    result.copy_from_slice(&data_buf[data_start..(track_size + data_start)]);
    Ok(result)
}

pub fn read_w3d_header(header_file: &Path) -> Result<Vec<u32>, String> {
    let mut file = File::open(header_file).map_err(|e| e.to_string())?;
    let mut buf = Vec::new();
    let size = file.read_to_end(&mut buf).map_err(|e| e.to_string())?;

    let num_headers = size / 4;
    let mut headers = Vec::with_capacity(num_headers);
    for i in 0..num_headers {
        let offset = u32::from_le_bytes(buf[(i * 4)..((i * 4) + 4)].try_into().unwrap());
        headers.push(offset)
    }

    Ok(headers)
}
