use clap::Parser;
use opl::catalog::w3d::{read_music_track, read_w3d_header};
use ratatui::{
    crossterm::{
        event::{self, Event, KeyCode, KeyEventKind},
        terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
        ExecutableCommand,
    },
    prelude::*,
    widgets::{Block, Borders, List, ListState, Paragraph},
};
use std::io::stdout;
use std::{
    env,
    fs::File,
    io::{self, Read},
    os::unix::fs::FileExt,
    path::{Path, PathBuf},
    str::FromStr,
};

#[derive(Parser)]
struct Cli {
    /// Path to the folder that contains the game files or
    /// a OPL file to play. If no path is supplied the cwd is taken.
    path: Option<std::path::PathBuf>,
}

struct Track {
    name: String,
}

struct TrackList {
    state: ListState,
    items: Vec<Track>,
}

struct App {
    tracks: TrackList,
}

pub fn main() -> Result<(), String> {
    let args = Cli::parse();

    let path = if let Some(path) = args.path {
        path
    } else {
        env::current_dir().map_err(|e| e.to_string())?
    };

    let track_data = if path.is_dir() {
        let headers = read_w3d_header(&path.join("AUDIOHED.WL6"))?;
        read_music_track(&headers, &path.join("AUDIOT.WL6"), 3)?
    } else {
        read_file(&path)
    };

    enable_raw_mode().map_err(|e| e.to_string())?;
    stdout()
        .execute(EnterAlternateScreen)
        .map_err(|e| e.to_string())?;
    let mut terminal = Terminal::new(CrosstermBackend::new(stdout())).map_err(|e| e.to_string())?;

    App::new().run(terminal).map_err(|e| e.to_string())?;

    disable_raw_mode().map_err(|e| e.to_string())?;
    stdout()
        .execute(LeaveAlternateScreen)
        .map_err(|e| e.to_string())?;
    Ok(())
}

impl App {
    fn new() -> App {
        App {
            tracks: TrackList {
                state: ListState::default(),
                items: vec![
                    Track {
                        name: "Wolfenstein 3D (1992)".to_string(),
                    },
                    Track {
                        name: "Duke Nukem (1996)".to_string(),
                    },
                    Track {
                        name: "Shadowcaster (1994)".to_string(),
                    },
                ],
            },
        }
    }

    fn run(&mut self, mut terminal: Terminal<impl Backend>) -> io::Result<()> {
        loop {
            self.draw(&mut terminal)?;

            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    match key.code {
                        KeyCode::Char('q') => return Ok(()),
                        KeyCode::Char('k') | KeyCode::Up => self.tracks.previous(),
                        KeyCode::Char('j') | KeyCode::Down => self.tracks.next(),
                        _ => {}
                    }
                }
            }
        }
    }

    fn draw(&mut self, terminal: &mut Terminal<impl Backend>) -> io::Result<()> {
        terminal.draw(|frame| frame.render_widget(self, frame.area()))?;
        Ok(())
    }
}

impl Widget for &mut App {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let [playback_area, tracks_area] =
            Layout::vertical([Constraint::Length(7), Constraint::Min(10)]).areas(area);

        Paragraph::new("TODO fill playback")
            .block(Block::bordered().title("Playback"))
            .render(playback_area, buf);

        let list = List::new(
            self.tracks
                .items
                .iter()
                .map(|track| track.name.clone())
                .collect::<Vec<String>>(),
        )
        .highlight_style(Style::new().add_modifier(Modifier::REVERSED))
        .block(Block::bordered().title("Tracks"));
        StatefulWidget::render(list, tracks_area, buf, &mut self.tracks.state);
    }
}

impl TrackList {
    fn previous(&mut self) {
        let i = match self.state.selected() {
            Some(i) => {
                if i == 0 {
                    self.items.len() - 1
                } else {
                    i - 1
                }
            }
            None => 0,
        };
        self.state.select(Some(i));
    }

    fn next(&mut self) {
        let i = match self.state.selected() {
            Some(i) => {
                if i >= self.items.len() - 1 {
                    0
                } else {
                    i + 1
                }
            }
            None => 0,
        };
        self.state.select(Some(i));
    }
}

fn opl(track_data: Vec<u8>) {
    let mut opl = opl::new().expect("opl setup");
    let settings = opl::OPLSettings {
        mixer_rate: 49716,
        imf_clock_rate: 0,
    };
    opl.play(track_data, settings).expect("play");

    let mut line = String::new();
    let _ = std::io::stdin()
        .read_line(&mut line)
        .expect("Failed to read line");
}

// Assumes a 'ripped AudioT chunk' as for now
fn read_file(file: &Path) -> Vec<u8> {
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
