use opl;

use std::time::Duration;

pub fn main() {
    let mut opl = opl::new().expect("opl setup");
    opl.start().expect("opl start");
    
    std::thread::sleep(Duration::from_millis(10000));
    println!("end");
}