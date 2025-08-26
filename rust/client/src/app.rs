use crate::command;
use crate::ui;
use color_eyre::Result;
use futures_channel::mpsc::UnboundedSender;
use ratatui::DefaultTerminal;
use ratatui::crossterm::event::{self, Event, KeyCode, KeyEventKind};
use shared::{ClientAction::*, TalkProtocol};
use std::sync::{Arc, Mutex};
use std::thread::sleep;
use std::time::{Duration, Instant};
use uuid::Uuid;

pub struct App {
    pub input: String,
    pub character_index: usize,
    pub input_mode: InputMode,
    pub scroll: usize,
    pub communication: Arc<Mutex<Vec<TalkProtocol>>>,
    pub tx: UnboundedSender<TalkProtocol>,
    pub username: String,
    pub room: i32,
}

pub enum InputMode {
    Normal,
    Editing,
}

impl App {
    pub fn new(
        transmit: UnboundedSender<TalkProtocol>,
        com: Arc<Mutex<Vec<TalkProtocol>>>,
    ) -> Self {
        Self {
            input: String::new(),
            input_mode: InputMode::Normal,
            communication: com,
            scroll: 0,
            character_index: 0,
            tx: transmit,
            username: "Client".to_string(),
            room: 0,
        }
    }

    fn join_initial_room(&mut self) {
        let com = TalkProtocol {
            uuid: Uuid::new_v4(),
            username: "Info".to_string(),
            message: Some(format!("{} joined the room", self.username)),
            action: Join,
            room_id: self.room,
            unixtime: command::get_unix_timestamp(),
        };
        self.tx.unbounded_send(com).unwrap();
    }

    fn move_cursor_left(&mut self) {
        let cursor_moved_left = self.character_index.saturating_sub(1);
        self.character_index = self.clamp_cursor(cursor_moved_left);
    }

    fn move_cursor_right(&mut self) {
        let cursor_moved_right = self.character_index.saturating_add(1);
        self.character_index = self.clamp_cursor(cursor_moved_right);
    }

    fn enter_char(&mut self, new_char: char) {
        let index = self.byte_index();
        self.input.insert(index, new_char);
        self.move_cursor_right();
    }

    fn byte_index(&self) -> usize {
        self.input
            .char_indices()
            .map(|(i, _)| i)
            .nth(self.character_index)
            .unwrap_or(self.input.len())
    }

    fn delete_char(&mut self) {
        let is_not_cursor_leftmost = self.character_index != 0;
        if is_not_cursor_leftmost {
            let current_index = self.character_index;
            let from_left_to_current_index = current_index - 1;
            let before_char_to_delete = self.input.chars().take(from_left_to_current_index);
            let after_char_to_delete = self.input.chars().skip(current_index);

            self.input = before_char_to_delete.chain(after_char_to_delete).collect();
            self.move_cursor_left();
        }
    }

    fn clamp_cursor(&self, new_cursor_pos: usize) -> usize {
        new_cursor_pos.clamp(0, self.input.chars().count())
    }

    fn reset_cursor(&mut self) {
        self.character_index = 0;
    }

    fn submit_message(&mut self) {
        command::parse(self);
        self.input.clear();
        self.reset_cursor();
    }

    pub fn run(mut self, mut terminal: DefaultTerminal) -> Result<()> {
        let tick_rate = Duration::from_millis(100);
        self.join_initial_room();
        loop {
            terminal.draw(|frame| self.draw(frame))?;

            let last_tick = Instant::now();
            let timeout = tick_rate
                .checked_sub(last_tick.elapsed())
                .unwrap_or(Duration::from_secs(0));

            if event::poll(timeout)? {
                if let Event::Key(key) = event::read()? {
                    match self.input_mode {
                        InputMode::Normal => match key.code {
                            KeyCode::Char('i') => {
                                self.input_mode = InputMode::Editing;
                            }
                            KeyCode::Char('q') => {
                                let message = format!("{} left the Chat", self.username);
                                let com = TalkProtocol {
                                    uuid: Uuid::new_v4(),
                                    username: "Info".to_string(),
                                    message: Some(message.to_string()),
                                    action: Leave,
                                    room_id: self.room,
                                    unixtime: command::get_unix_timestamp(),
                                };
                                self.tx.unbounded_send(com).unwrap();
                                return Ok(());
                            }
                            KeyCode::Char('k') => {
                                if self.scroll + 1 < self.communication.lock().unwrap().len() {
                                    self.scroll += 1;
                                }
                            }
                            KeyCode::Char('j') => {
                                if self.scroll > 0 {
                                    self.scroll -= 1;
                                }
                            }
                            _ => {}
                        },
                        InputMode::Editing if key.kind == KeyEventKind::Press => match key.code {
                            KeyCode::Enter => self.submit_message(),
                            KeyCode::Char(to_insert) => self.enter_char(to_insert),
                            KeyCode::Backspace => self.delete_char(),
                            KeyCode::Left => self.move_cursor_left(),
                            KeyCode::Right => self.move_cursor_right(),
                            KeyCode::Esc => self.input_mode = InputMode::Normal,
                            _ => {}
                        },
                        InputMode::Editing => {}
                    }
                }
            }
        }
    }

    fn draw(&mut self, frame: &mut ratatui::Frame) {
        ui::draw(self, frame);
    }
}
