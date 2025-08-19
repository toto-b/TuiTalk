use futures_channel::mpsc::{UnboundedSender, unbounded};
pub use shared::native::{connect, receiver_task, sender_task};
use shared::{
    ClientAction::{Join, Leave, Send},
    TalkProtocol,
};
use std::env;
use std::sync::Arc;
use std::sync::Mutex;
use uuid::Uuid;

use color_eyre::Result;
use ratatui::{
    DefaultTerminal, Frame,
    crossterm::event::{self, Event, KeyCode, KeyEventKind},
    layout::{Constraint, Layout, Position},
    style::{Color, Modifier, Style, Stylize},
    text::{Line, Span, Text},
    widgets::{Block, List, ListItem, Paragraph},
};
use std::time::{Duration, Instant};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let url = env::args()
        .nth(1)
        .unwrap_or_else(|| "ws://0.0.0.0:8080".to_string());

    let (tx, rx) = unbounded::<TalkProtocol>();
    let (write, read) = connect(url).await?;
    let communication: Arc<Mutex<Vec<TalkProtocol>>> = Arc::new(Mutex::new(Vec::new()));
    tokio::spawn(sender_task(rx, write));

    let com = Arc::clone(&communication);
    tokio::spawn(receiver_task(read, move |msg| {
        com.lock().unwrap().push(msg);
    }));

    color_eyre::install()?;
    let terminal = ratatui::init();
    let app_result = App::new(tx, communication).run(terminal);
    ratatui::restore();
    Ok(app_result?)
}

fn send_message(tx: UnboundedSender<TalkProtocol>, communication_protocol: TalkProtocol) {
    tx.unbounded_send(communication_protocol).unwrap();
}

struct App {
    input: String,
    character_index: usize,
    input_mode: InputMode,
    scroll: usize,
    communication: Arc<Mutex<Vec<TalkProtocol>>>,
    tx: UnboundedSender<TalkProtocol>,
}

enum InputMode {
    Normal,
    Editing,
}

impl App {
    const fn new(
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
        }
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
        let com = TalkProtocol {
            uuid: Uuid::new_v4(),
            username: "Client".to_string(),
            message: Some(self.input.clone()),
            action: Send,
            room_id: 1,
            unixtime: 2,
        };
        send_message(self.tx.clone(), com.clone());

        self.input.clear();
        self.reset_cursor();
    }

    fn run(mut self, mut terminal: DefaultTerminal) -> Result<()> {
        let tick_rate = Duration::from_millis(100);
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

    fn draw(&mut self, frame: &mut Frame) {
        let vertical = Layout::vertical([
            Constraint::Length(1),
            Constraint::Length(3),
            Constraint::Min(1),
        ]);
        let [help_area, input_area, messages_area] = vertical.areas(frame.area());

        let (msg, style) = match self.input_mode {
            InputMode::Normal => (
                vec![
                    "Press ".into(),
                    "q".bold(),
                    " to exit, ".into(),
                    "i".bold(),
                    " to start editing.".bold(),
                ],
                Style::default().add_modifier(Modifier::RAPID_BLINK),
            ),
            InputMode::Editing => (
                vec![
                    "Press ".into(),
                    "Esc".bold(),
                    " to stop editing, ".into(),
                    "Enter".bold(),
                    " to record the message".into(),
                ],
                Style::default(),
            ),
        };
        let text = Text::from(Line::from(msg)).patch_style(style);
        let help_message = Paragraph::new(text);
        frame.render_widget(help_message, help_area);

        let input = Paragraph::new(self.input.as_str())
            .style(match self.input_mode {
                InputMode::Normal => Style::default(),
                InputMode::Editing => Style::default().fg(Color::Yellow),
            })
            .block(Block::bordered().title("Input"));
        frame.render_widget(input, input_area);
        match self.input_mode {
            InputMode::Normal => {}

            #[allow(clippy::cast_possible_truncation)]
            InputMode::Editing => frame.set_cursor_position(Position::new(
                input_area.x + self.character_index as u16 + 1,
                input_area.y + 1,
            )),
        }

        let full_messages = self.communication.lock().unwrap();
        let height = messages_area.height as usize;

        if self.scroll > full_messages.len().saturating_sub(height - 2) {
            self.scroll = full_messages.len().saturating_sub(height - 2);
        }

        let start = full_messages.len().saturating_sub(height - 2 + self.scroll);
        let visible = &full_messages[start..];

        let communication: Vec<ListItem> = visible
            .iter()
            .map(|m| {
                let content = Line::from(Span::raw(format!(
                    "{}: {}",
                    m.username,
                    m.message.clone().expect("Message in Option")
                )));
                ListItem::new(content)
            })
            .collect();

        let communication = List::new(communication).block(Block::bordered().title("Messages"));
        frame.render_widget(communication, messages_area);
    }
}
