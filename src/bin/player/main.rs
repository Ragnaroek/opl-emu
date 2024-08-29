use clap::Parser;
use opl::backend_sdl::OPL;
use opl::catalog::{GameModule, Track, CATALOGED_GAMES};
use ratatui::{
    crossterm::{
        event::{self, Event, KeyCode, KeyEventKind},
        terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
        ExecutableCommand,
    },
    prelude::*,
    style::palette::tailwind::SLATE,
    widgets::{Block, List, ListState, Paragraph},
};
use std::io::{self, stdout};
use std::path::Path;

const FOCUS_SELECTED_STYLE: Style = Style::new()
    .bg(SLATE.c100)
    .fg(SLATE.c950)
    .add_modifier(Modifier::BOLD);
const UNFOCUS_SELECTED_STYLE: Style = Style::new().bg(SLATE.c500).add_modifier(Modifier::BOLD);

#[derive(Parser)]
struct Cli {
    /// Path to the folder that contains the game files or
    /// a OPL file to play. If no path is supplied the cwd is taken.
    path: Option<std::path::PathBuf>,
}

struct PlayState {
    game: &'static GameModule,
    track: &'static Track,
}

struct State {
    game_state: ListState,
    track_state: ListState,
    focus_state: Focused,
    play_state: Option<PlayState>,
    opl: OPL,
}

struct App {
    state: State,
}

pub fn main() -> Result<(), String> {
    let args = Cli::parse();

    enable_raw_mode().map_err(|e| e.to_string())?;
    stdout()
        .execute(EnterAlternateScreen)
        .map_err(|e| e.to_string())?;
    let terminal = Terminal::new(CrosstermBackend::new(stdout())).map_err(|e| e.to_string())?;

    let opl = opl::new()?;
    App::new(opl).run(terminal).map_err(|e| e.to_string())?;

    disable_raw_mode().map_err(|e| e.to_string())?;
    stdout()
        .execute(LeaveAlternateScreen)
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[derive(Copy, Clone, PartialEq)]
enum Focused {
    GameList,
    TrackList,
}

impl App {
    fn new(opl: OPL) -> App {
        App {
            state: State {
                game_state: ListState::default(),
                track_state: ListState::default(),
                focus_state: Focused::GameList,
                opl,
                play_state: None,
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
                        KeyCode::Char('k') | KeyCode::Up => {
                            if self.state.focus_state == Focused::GameList {
                                list_previous(&mut self.state.game_state, CATALOGED_GAMES.len())
                            } else {
                                if let Some(game_selected) = self.state.game_state.selected() {
                                    let len = CATALOGED_GAMES[game_selected].metadata.tracks.len();
                                    list_previous(&mut self.state.track_state, len)
                                }
                            }
                        }
                        KeyCode::Char('j') | KeyCode::Down => {
                            if self.state.focus_state == Focused::GameList {
                                list_next(&mut self.state.game_state, CATALOGED_GAMES.len())
                            } else {
                                if let Some(game_selected) = self.state.game_state.selected() {
                                    let len = CATALOGED_GAMES[game_selected].metadata.tracks.len();
                                    list_next(&mut self.state.track_state, len)
                                }
                            }
                        }
                        KeyCode::Tab => {
                            if self.state.focus_state == Focused::GameList {
                                self.state.focus_state = Focused::TrackList;
                                self.state.track_state.select(Some(0));
                            } else {
                                self.state.focus_state = Focused::GameList;
                            }
                        }
                        KeyCode::Enter => {
                            if self.state.focus_state == Focused::GameList {
                                if let Some(game_selected) = self.state.game_state.selected() {
                                    self.state.play_state = Some(PlayState {
                                        track: &CATALOGED_GAMES[game_selected].metadata.tracks[0],
                                        game: CATALOGED_GAMES[game_selected],
                                    });
                                }
                            } else {
                                if let Some(game_selected) = self.state.game_state.selected() {
                                    if let Some(track_selected) = self.state.track_state.selected()
                                    {
                                        self.state.play_state = Some(PlayState {
                                            track: &CATALOGED_GAMES[game_selected].metadata.tracks
                                                [track_selected],
                                            game: CATALOGED_GAMES[game_selected],
                                        });
                                    }
                                }
                            }

                            if let Some(play_state) = &self.state.play_state {
                                // TODO remove hard-coded path and replace it with a config/scan result
                                // TODO Take OPL_Settings from GameModule config?
                                // TODO Remove expected and update playState with error
                                let settings = opl::OPLSettings {
                                    mixer_rate: 49716,
                                    imf_clock_rate: 0,
                                };
                                let track_data = (play_state.game.track_loader)(
                                    Path::new("/Users/michaelbohn/_w3d/w3d_data"),
                                    play_state.track.no,
                                )
                                .expect("load track data");
                                self.state.opl.play(track_data, settings).expect("opl play");
                            }
                        }
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
        let [playback_area, bottom_area] =
            Layout::vertical([Constraint::Length(7), Constraint::Min(10)]).areas(area);

        let [game_area, track_area] =
            Layout::horizontal([Constraint::Percentage(50), Constraint::Percentage(50)])
                .areas(bottom_area);

        let playback_str = if let Some(play_state) = &self.state.play_state {
            "Playing: ".to_string() + play_state.track.name
        } else {
            "Nothing selected to play".to_string()
        };

        Paragraph::new(playback_str)
            .block(Block::bordered().title("Playback"))
            .render(playback_area, buf);

        let game_list = List::new(
            CATALOGED_GAMES
                .iter()
                .map(|game| game.metadata.name)
                .collect::<Vec<&'static str>>(),
        )
        .highlight_style(highlight_style(Focused::GameList, self.state.focus_state))
        .block(Block::bordered().title("Games"));
        StatefulWidget::render(game_list, game_area, buf, &mut self.state.game_state);

        let track_list = if let Some(selected) = self.state.game_state.selected() {
            let tracks = &CATALOGED_GAMES[selected].metadata.tracks;
            tracks
                .iter()
                .map(|track| track.name)
                .collect::<Vec<&'static str>>()
        } else {
            Vec::<&'static str>::new()
        };

        let track_list = List::new(track_list)
            .highlight_style(highlight_style(Focused::TrackList, self.state.focus_state))
            .block(Block::bordered().title("Tracks"));
        StatefulWidget::render(track_list, track_area, buf, &mut self.state.track_state);
    }
}

fn highlight_style(want_focus: Focused, has_focus: Focused) -> Style {
    if want_focus == has_focus {
        FOCUS_SELECTED_STYLE
    } else {
        UNFOCUS_SELECTED_STYLE
    }
}

fn list_previous(list_state: &mut ListState, max_len: usize) {
    let i = match list_state.selected() {
        Some(i) => {
            if i == 0 {
                max_len - 1
            } else {
                i - 1
            }
        }
        None => 0,
    };
    list_state.select(Some(i));
}

fn list_next(list_state: &mut ListState, max_len: usize) {
    let i = match list_state.selected() {
        Some(i) => {
            if i >= max_len - 1 {
                0
            } else {
                i + 1
            }
        }
        None => 0,
    };
    list_state.select(Some(i));
}
