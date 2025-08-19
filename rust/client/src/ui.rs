use crate::app::{App, InputMode};
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Position},
    style::{Color, Modifier, Style, Stylize},
    text::{Line, Span, Text},
    widgets::{Block, List, ListItem, Paragraph},
};

pub fn draw(app: &mut App, frame: &mut Frame) {
    let vertical = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(3),
        Constraint::Min(1),
    ]);
    let [help_area, input_area, messages_area] = vertical.areas(frame.area());

    // Help bar
    let (msg, style) = match app.input_mode {
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
                " to send".into(),
            ],
            Style::default(),
        ),
    };
    let text = Text::from(Line::from(msg)).patch_style(style);
    frame.render_widget(Paragraph::new(text), help_area);

    // Input field
    let input = Paragraph::new(app.input.as_str())
        .style(match app.input_mode {
            InputMode::Normal => Style::default(),
            InputMode::Editing => Style::default().fg(Color::Yellow),
        })
        .block(Block::bordered().title("Input"));
    frame.render_widget(input, input_area);

    if let InputMode::Editing = app.input_mode {
        frame.set_cursor_position(Position::new(
            input_area.x + app.character_index as u16 + 1,
            input_area.y + 1,
        ));
    }

    // Messages
    let full_messages = app.communication.lock().unwrap();
    let height = messages_area.height as usize;
    if app.scroll > full_messages.len().saturating_sub(height - 2) {
        app.scroll = full_messages.len().saturating_sub(height - 2);
    }

    let start = full_messages.len().saturating_sub(height - 2 + app.scroll);
    let visible = &full_messages[start..];

    let communication: Vec<ListItem> = visible
        .iter()
        .map(|m| {
            let content = Line::from(Span::raw(format!(
                "{}: {}",
                m.username,
                m.message.clone().unwrap_or_default()
            )));
            ListItem::new(content)
        })
        .collect();

    let communication = List::new(communication).block(Block::bordered().title("Messages"));
    frame.render_widget(communication, messages_area);
}
