
use sdl2::{self, AudioSubsystem};
use sdl2::audio::{AudioCallback, AudioDevice, AudioSpecDesired};

pub struct OPL {
    audio_subsystem: AudioSubsystem,
    device: Option<AudioDevice<SquareWave>>
}

pub fn new() -> Result<OPL, &'static str>  {
    let sdl_context = sdl2::init().expect("sdl init failed");
    let audio_subsystem = sdl_context.audio().expect("audio init failed");
    Ok(OPL{
        audio_subsystem,
        device: None
    })
}

impl OPL {
    pub fn start(&mut self) -> Result<(), &'static str> {
        let desired_spec = AudioSpecDesired {
            freq: Some(44100),
            channels: Some(2),  
            samples: Some(2048),       
        };

        let device = self.audio_subsystem.open_playback(None, &desired_spec, |spec| {
            // initialize the audio callback
            SquareWave {
                phase_inc: 440.0 / spec.freq as f32,
                phase: 0.0,
                volume: 0.25
            }
        }).expect("playback open failed");
        device.resume();
        println!("playback started on device: {}", device.subsystem().current_audio_driver());
        self.device = Some(device);
        Ok(())
    }
}

struct SquareWave {
    phase_inc: f32,
    phase: f32,
    volume: f32
}

impl AudioCallback for SquareWave {
    type Channel = f32;

    fn callback(&mut self, out: &mut [f32]) {
        // Generate a square wave
        for x in out.iter_mut() {
            *x = if self.phase <= 0.5 {
                self.volume
            } else {
                -self.volume
            };
            self.phase = (self.phase + self.phase_inc) % 1.0;
        }
    }
}