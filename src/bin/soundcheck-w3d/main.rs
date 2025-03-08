use clap::Parser;
use opl::{
    AdlSound, Instrument,
    catalog::w3d::{AUDIO_FILE, HEADER_FILE, START_SOUND, load_chunk, load_track, read_w3d_header},
};
use ratatui::DefaultTerminal;
use ratatui::crossterm::{
    event::{self, Event, KeyCode, KeyEventKind},
    terminal::enable_raw_mode,
};
use ratatui::prelude::*;
use ratatui::widgets::Paragraph;
use std::{env, path::PathBuf, str};

#[derive(Parser)]
struct Cli {
    /// Path to the folder that contains the game files. If non
    /// is supplied the cwd is taken.
    #[arg(short, long)]
    folder: Option<std::path::PathBuf>,
    /// sound chunk no to play in a loop
    sound_no: usize,
}

#[derive(PartialEq)]
enum Show {
    Hint,
    SoundSelect,
    TrackSelect,
}

struct State {
    show: Show,
    opl: opl::OPL,
    headers: Vec<u32>,
    sound_no: usize,
    track_no: usize,
    folder_path: PathBuf,
    num_input: String,
}

struct App {
    state: State,
}

// Test program, mainly created to test the mixing of OPL music track playing
// with ADL sound effects.
pub fn main() -> Result<(), String> {
    let args = Cli::parse();

    let folder_path = if let Some(path) = args.folder {
        path
    } else {
        env::current_dir().map_err(|e| e.to_string())?
    };

    let track_no = 7;
    let music_track = load_track(&folder_path, track_no).expect("music track");
    let headers = read_w3d_header(&folder_path.join(HEADER_FILE))?;

    let mut opl = opl::new()?;
    // set up the OPL with frequencies to play W3D sounds
    let settings = opl::OPLSettings {
        mixer_rate: 49716,
        imf_clock_rate: 700,
        adl_clock_rate: 140,
    };
    opl.init(settings);

    opl.play_imf(music_track)?;

    enable_raw_mode().map_err(|e| e.to_string())?;
    let terminal = ratatui::init();
    App::new(opl, headers, args.sound_no, track_no, folder_path)
        .run(terminal)
        .map_err(|e| e.to_string())?;
    ratatui::restore();

    Ok(())
}

impl App {
    fn new(
        opl: opl::OPL,
        headers: Vec<u32>,
        initial_sound_no: usize,
        initial_track_no: usize,
        folder_path: PathBuf,
    ) -> App {
        App {
            state: State {
                show: Show::Hint,
                opl,
                headers,
                sound_no: initial_sound_no,
                track_no: initial_track_no,
                folder_path,
                num_input: "".to_string(),
            },
        }
    }

    fn run(&mut self, mut terminal: DefaultTerminal) -> Result<(), String> {
        loop {
            self.draw(&mut terminal)?;

            let evt = event::read().map_err(|e| e.to_string())?;
            if let Event::Key(key) = evt {
                if key.kind == KeyEventKind::Press {
                    match key.code {
                        KeyCode::Char('q') => break,
                        KeyCode::Char('p') => {
                            //TODO add SOUND_START to chunk_no!
                            let chunk = load_chunk(
                                &self.state.headers,
                                &self.state.folder_path.join(AUDIO_FILE),
                                START_SOUND + self.state.sound_no,
                                0,
                            )?;
                            let sound = read_sound(chunk);
                            self.state.opl.play_adl(sound)?
                        }
                        KeyCode::Char('s') => {
                            self.state.show = Show::SoundSelect;
                        }
                        KeyCode::Char('t') => {
                            self.state.show = Show::TrackSelect;
                        }
                        KeyCode::Char(ch) => {
                            if self.input_mode() && ch.is_digit(10) {
                                self.state.num_input.push(ch);
                            }
                        }
                        KeyCode::Enter => {
                            let no_res = self.state.num_input.parse();
                            if no_res.is_ok() {
                                let no = no_res.expect("number");
                                if self.state.show == Show::SoundSelect {
                                    self.state.sound_no = no;
                                } else if self.state.show == Show::TrackSelect {
                                    self.state.track_no = no;
                                    let music_track =
                                        load_track(&self.state.folder_path, self.state.track_no)
                                            .expect("music track");
                                    self.state.opl.play_imf(music_track).expect("imf playing");
                                }
                                self.state.num_input = "".to_string();
                                self.state.show = Show::Hint;
                            }
                        }
                        _ => { /* ignore */ }
                    }
                }
            }
        }
        Ok(())
    }

    fn input_mode(&self) -> bool {
        self.state.show == Show::TrackSelect || self.state.show == Show::SoundSelect
    }

    fn draw(&mut self, terminal: &mut Terminal<impl Backend>) -> Result<(), String> {
        terminal
            .draw(|frame| frame.render_widget(self, frame.area()))
            .map_err(|e| e.to_string())?;
        Ok(())
    }
}

impl Widget for &mut App {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let p = match self.state.show {
            Show::Hint => Paragraph::new(format!(
                "Press 'q' to quit, 'p' to play sound number {} or 's'/'t' to choose a sound/track",
                self.state.sound_no
            )),
            Show::SoundSelect => {
                Paragraph::new(format!("Enter sound number: {}", &self.state.num_input))
            }
            Show::TrackSelect => {
                Paragraph::new(format!("Enter track number: {}", &self.state.num_input))
            }
        };
        p.render(area, buf);
    }
}

fn read_sound(data: Vec<u8>) -> AdlSound {
    let length = u32::from_le_bytes(data[0..4].try_into().unwrap());
    let instrument = Instrument {
        m_char: data[6],
        c_char: data[7],
        m_scale: data[8],
        c_scale: data[9],
        m_attack: data[10],
        c_attack: data[11],
        m_sus: data[12],
        c_sus: data[13],
        m_wave: data[14],
        c_wave: data[15],
        n_conn: data[16],
        voice: data[17],
        mode: data[18],
        // data[19..22] are padding and omitted
    };
    AdlSound {
        length,
        priority: u16::from_le_bytes(data[4..6].try_into().unwrap()),
        instrument,
        block: data[22],
        data: data[23..(23 + length as usize)].to_vec(),
        terminator: data[23 + length as usize],
        name: str::from_utf8(&data[(23 + length as usize) + 1..data.len() - 1])
            .expect("sound name")
            .to_string(),
    }
}
