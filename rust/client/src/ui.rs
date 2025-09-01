use crate::app::{App, InputMode};
use chrono::{Local, TimeZone, Utc};
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Position},
    style::{Color, Modifier, Style, Stylize},
    text::{Line, Span, Text},
    widgets::{Block, List, ListItem, Paragraph, Wrap},
};
use uuid::Uuid;

fn color_from_uuid(username: &String, uuid: Uuid) -> Color {
    if username == "Info" {
        Color::Yellow
    } else if username == "Error" {
        Color::Red
    } else {
        let bytes = uuid.as_bytes();
        let r = bytes[0].saturating_add(64);
        let g = bytes[1].saturating_add(64);
        let b = bytes[2].saturating_add(64);

        Color::Rgb(r, g, b)
    }
}

pub fn draw(app: &mut App, frame: &mut Frame) {
    let vertical = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(3),
        Constraint::Min(1),
    ]);
    let [help_area, input_area, messages_area] = vertical.areas(frame.area());

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

    let full_messages = app.communication.lock().unwrap();
    let height = messages_area.height as usize;
    if app.scroll > full_messages.len().saturating_sub(height - 2) {
        app.scroll = full_messages.len().saturating_sub(height - 2);
    }

    let start = full_messages.len().saturating_sub(height - 2 + app.scroll);
    let visible = &full_messages[start..];

    let communication: Vec<Line> = visible
        .iter()
        .map(|m| {
            let timestamp = Span::raw(format!(
                "<{}> ",
                Utc.timestamp_opt(m.unixtime as i64, 0)
                    .unwrap()
                    .with_timezone(&Local)
                    .format("%H:%M")
            ));

            let username = Span::styled(
                format!("{}: ", m.username),
                Style::default().fg(color_from_uuid(&m.username, m.uuid)),
            );

            let message = Span::raw(m.message.clone().unwrap_or_default());

            let content = Line::from(vec![timestamp, username, message]);
            Line::from(content)
        })
        .collect();

    let communication = Paragraph::new(communication)
        .block(Block::bordered().title("Messages"))
        .wrap(Wrap { trim: true });
    frame.render_widget(communication, messages_area);
}
