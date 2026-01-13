use std::io::{Cursor, Read, Seek, SeekFrom};
use std::{fs::File, os::unix::fs::FileExt, path::Path};

use super::util::DataReader;
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

pub static AUDIO_HEADER_FILE: &str = "AUDIOHED.WL6";
pub static AUDIO_FILE: &str = "AUDIOT.WL6";
pub static GAMEDATA_FILE: &str = "VSWAP.WL6";

pub const START_ADLIB_SOUND: usize = 87;
pub const START_MUSIC: usize = 261;

// Game Module interface
fn is_w3d() -> bool {
    todo!("check w3d folder structure");
}

pub fn load_track(game_path: &Path, track_no: usize) -> Result<Vec<u8>, String> {
    let headers = read_w3d_audio_header(&game_path.join(AUDIO_HEADER_FILE))?;
    let track_chunk = load_audio_chunk(
        &headers,
        &game_path.join(AUDIO_FILE),
        START_MUSIC + track_no,
    )?;

    let track_size = u16::from_le_bytes(track_chunk[0..2].try_into().unwrap()) as usize;
    let mut result = vec![0; track_size];
    result.copy_from_slice(&track_chunk[2..(track_size + 2)]);
    Ok(result)
}

pub fn load_sound(game_path: &Path, sound_no: usize) -> Result<Vec<u8>, String> {
    let headers = read_w3d_audio_header(&game_path.join(AUDIO_HEADER_FILE))?;
    load_audio_chunk(
        &headers,
        &game_path.join(AUDIO_FILE),
        START_ADLIB_SOUND + sound_no,
    )
}
// End Game Module interface

// Extra interface

pub fn load_digi(game_path: &Path, digi_no: usize) -> Result<Vec<u8>, String> {
    let gamedata_path = game_path.join(GAMEDATA_FILE);
    let mut gamedata_file = File::open(&gamedata_path).map_err(|e| e.to_string())?;
    let mut gamedata_bytes = Vec::new();
    gamedata_file
        .read_to_end(&mut gamedata_bytes)
        .map_err(|e| e.to_string())?;

    let headers = read_w3d_gamedata_header(&gamedata_bytes)?;
    let mut gamedata_cursor = Cursor::new(gamedata_bytes);
    let sound_info_page = load_page(
        &mut gamedata_cursor,
        &headers,
        (headers.num_chunks - 1) as usize,
    )?;

    let start_page = u16::from_le_bytes(
        sound_info_page[(digi_no * 4)..(digi_no * 4 + 2)]
            .try_into()
            .unwrap(),
    ) as usize;
    let length = u16::from_le_bytes(
        sound_info_page[(digi_no * 4 + 2)..(digi_no * 4 + 4)]
            .try_into()
            .unwrap(),
    ) as usize;

    load_digi_page(&mut gamedata_cursor, &headers, start_page, length)
}

// End extra interface

pub struct GamedataHeader {
    pub offset: u32,
    pub length: u16,
}

pub struct GamedataHeaders {
    pub num_chunks: u16,
    pub sprite_start: u16,
    pub sound_start: u16,
    pub headers: Vec<GamedataHeader>,
}

fn load_digi_page<M: Read + Seek>(
    data: &mut M,
    headers: &GamedataHeaders,
    start_page: usize,
    length: usize,
) -> Result<Vec<u8>, String> {
    let header = &headers.headers[headers.sound_start as usize + start_page];
    data.seek(SeekFrom::Start(header.offset as u64))
        .map_err(|e| e.to_string())?;
    let mut buffer: Vec<u8> = vec![0; length as usize];
    let n = data.read(&mut buffer).map_err(|e| e.to_string())?;
    if n != length as usize {
        return Err("not enough bytes in page".to_string());
    }
    Ok(buffer)
}

/// Reads a raw chunk from the W3D audiofile.
pub fn load_audio_chunk(
    headers: &Vec<u32>,
    audiot_file: &Path,
    chunk_no: usize,
) -> Result<Vec<u8>, String> {
    let file = File::open(audiot_file).map_err(|e| e.to_string())?;
    let offset = headers[chunk_no];
    let size = (headers[chunk_no + 1] - offset) as usize;

    let mut data_buf = vec![0; size];
    file.read_exact_at(&mut data_buf, offset as u64)
        .map_err(|e| e.to_string())?;
    Ok(data_buf)
}

fn load_page<M: Read + Seek>(
    data: &mut M,
    headers: &GamedataHeaders,
    page: usize,
) -> Result<Vec<u8>, String> {
    let header = &headers.headers[page];
    data.seek(SeekFrom::Start(header.offset as u64))
        .map_err(|e| e.to_string())?;
    let mut buffer: Vec<u8> = vec![0; header.length as usize];
    let n = data.read(&mut buffer).map_err(|e| e.to_string())?;
    if n != header.length as usize {
        return Err("not enough bytes in page".to_string());
    }
    Ok(buffer)
}

pub fn read_w3d_audio_header(header_file: &Path) -> Result<Vec<u32>, String> {
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

pub fn read_w3d_gamedata_header(gamedata_bytes: &[u8]) -> Result<GamedataHeaders, String> {
    let mut reader = DataReader::new(&gamedata_bytes);
    let num_chunks = reader.read_u16();
    let sprite_start = reader.read_u16();
    let sound_start = reader.read_u16();

    let mut headers = Vec::with_capacity(num_chunks as usize);
    for _ in 0..num_chunks {
        let offset = reader.read_u32();
        headers.push(GamedataHeader { offset, length: 0 });
    }

    for i in 0..num_chunks as usize {
        let length = reader.read_u16();
        headers[i].length = length;
    }

    Ok(GamedataHeaders {
        num_chunks,
        sprite_start,
        sound_start,
        headers,
    })
}
