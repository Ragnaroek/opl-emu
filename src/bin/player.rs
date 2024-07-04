use opl;

use std::{env, fs::File, io::Read, os::unix::fs::FileExt, time::Duration};

pub fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() != 2 {
        print_usage(&args[0]);
        return;
    }
    let data = read_file(&args[1]);

    println!(
        "data len = {}, byte0={}, byte1={}",
        data.len(),
        data[0],
        data[1]
    );

    let settings = opl::OPLSettings {
        mixer_rate: 49716,
        imf_clock_rate: 0,
    };

    let mut opl = opl::new().expect("opl setup");
    opl.play(data, settings).expect("play");

    std::thread::sleep(Duration::from_millis(10000));
    println!("end");
}

// Assumes a 'ripped AudioT chunk' as for now
fn read_file(file: &str) -> Vec<u8> {
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

fn print_usage(arg_0: &str) {
    println!("Usage:");
    println!("{} <file>", arg_0);
}
