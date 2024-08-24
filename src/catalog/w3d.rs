use std::{fs::File, io::Read, os::unix::fs::FileExt, path::Path};

pub struct W3DHeaders {
    pub start_music: usize,
    pub headers: Vec<u32>,
}

pub fn read_w3d_header(header_file: &Path) -> Result<W3DHeaders, String> {
    let mut file = File::open(header_file).map_err(|e| e.to_string())?;
    let mut buf = Vec::new();
    let size = file.read_to_end(&mut buf).map_err(|e| e.to_string())?;

    let num_headers = size / 4;
    let mut headers = Vec::with_capacity(num_headers);
    for i in 0..num_headers {
        let offset = u32::from_le_bytes(buf[(i * 4)..((i * 4) + 4)].try_into().unwrap());
        headers.push(offset)
    }

    Ok(W3DHeaders {
        start_music: 261,
        headers,
    })
}

pub fn read_music_track(
    headers: &W3DHeaders,
    audiot_file: &Path,
    track_no: usize,
) -> Result<Vec<u8>, String> {
    let file = File::open(audiot_file).map_err(|e| e.to_string())?;
    let offset = headers.headers[headers.start_music + track_no];
    let size = (headers.headers[headers.start_music + track_no + 1] - offset) as usize;

    let mut data_buf = vec![0; size - 2];
    file.read_exact_at(&mut data_buf, (offset + 2) as u64)
        .map_err(|e| e.to_string())?;
    Ok(data_buf)
}
