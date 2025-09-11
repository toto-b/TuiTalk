use crate::app::{App, InputMode};
use anyhow::{Context, Result};
use chrono::{Local, TimeZone, Utc};
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Position},
    style::{Color, Modifier, Style, Stylize},
    text::{Line, Span, Text},
    widgets::{Block, Paragraph, Wrap},
};
use shared::*;
use uuid::Uuid;

fn color_from_uuid(uuid: Uuid) -> Color {
    let bytes = uuid.as_bytes();
    let r = bytes[0].saturating_add(64);
    let g = bytes[1].saturating_add(64);
    let b = bytes[2].saturating_add(64);

    Color::Rgb(r, g, b)
}

fn format_timestamp(unixtime: u64) -> Result<Span<'static>> {
    let timestamp = Utc
        .timestamp_opt(unixtime as i64, 0)
        .single()
        .context("Invalid Timestamp")?;
    Ok(Span::raw(format!(
        "<{}> ",
        timestamp.with_timezone(&Local).format("%H:%M")
    )))
}

fn return_server_error(message: &String, code: &String) -> Result<Line<'static>> {
    let error = Span::styled(format!("Server Error"), Style::default().fg(Color::Red));
    let code = Span::raw(code.clone());
    let space = Span::raw(": ".to_string());

    let message = Span::raw(message.clone());

    let content = Line::from(vec![error, space, code, message]);
    Ok(content)
}

fn return_local_error(message: &String) -> Result<Line> {
    let error = Span::styled(format!("Local Error"), Style::default().fg(Color::Red));
    let space = Span::raw(": ".to_string());

    let message = Span::raw(message);

    let content = Line::from(vec![error, space, message]);
    Ok(content)
}

fn return_user_left(unixtime: u64, username: &String, uuid: Uuid) -> Result<Line> {
    let timestamp = format_timestamp(unixtime)?;
    let info = Span::styled(format!("Info: "), Style::default().fg(Color::Yellow));
    let username = Span::styled(username, Style::default().fg(color_from_uuid(uuid)));

    let message = Span::raw(" left the room");

    let content = Line::from(vec![timestamp, info, username, message]);
    Ok(content)
}

fn return_user_joined(unixtime: u64, username: &String, uuid: Uuid) -> Result<Line> {
    let timestamp = format_timestamp(unixtime)?;

    let info = Span::styled(format!("Info: "), Style::default().fg(Color::Yellow));
    let username = Span::styled(username, Style::default().fg(color_from_uuid(uuid)));

    let message = Span::raw(" joined the room");

    let content = Line::from(vec![timestamp, info, username, message]);
    Ok(content)
}

fn return_username_changed<'a>(
    unixtime: u64,
    username: &String,
    old_username: &String,
    uuid: Uuid,
) -> Result<Line<'static>> {
    let timestamp = format_timestamp(unixtime)?;

    let info = Span::styled("Info: ".to_string(), Style::default().fg(Color::Yellow));
    let old_username = Span::styled(old_username.clone(), Style::default().fg(color_from_uuid(uuid)));

    let message = Span::raw(" changed his name to ");
    let username = Span::styled(username.clone(), Style::default().fg(color_from_uuid(uuid)));

    let content = Line::from(vec![timestamp, info, old_username, message, username]);
    Ok(content)
}

fn return_posted_message(message: &TalkMessage) -> Result<Line> {
    let timestamp = format_timestamp(message.unixtime)?;

    let username = Span::styled(
        format!("{}: ", message.username),
        Style::default().fg(color_from_uuid(message.uuid)),
    );

    let message = Span::raw(message.text.clone());

    let content = Line::from(vec![timestamp, username, message]);
    Ok(content)
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

    let full_messages = app.communication.lock().expect("Vector with all messages");

    let lines: Vec<Line> = full_messages
        .iter()
        .map(|proto| match proto {
            TalkProtocol::Error { code, message } => return_server_error(message, code),
            TalkProtocol::LocalError { message } => return_local_error(message),
            TalkProtocol::PostMessage { message } => return_posted_message(message),
            TalkProtocol::UserJoined {
                uuid,
                username,
                room_id: _,
                unixtime,
            } => return_user_joined(*unixtime, username, uuid.clone()),
            TalkProtocol::UserLeft {
                uuid,
                username,
                room_id: _,
                unixtime,
            } => return_user_left(*unixtime, username, uuid.clone()),
            TalkProtocol::UsernameChanged {
                uuid,
                username,
                old_username,
                unixtime,
            } => return_username_changed(*unixtime, username, old_username, uuid.clone()),
            _ => Ok(Line::from(Span::raw(format!("{:?}", proto)))),
        })
        .collect::<Result<Vec<Line>, anyhow::Error>>().expect("lines of text");
    let paragraph = Paragraph::new(lines).wrap(Wrap { trim: true });

    let total_lines = paragraph.line_count(messages_area.width.into());
    let visible_height = messages_area.height.saturating_sub(2) as usize;
    app.max_scroll = total_lines.saturating_sub(visible_height);

    if app.auto_scroll {
        app.scroll = total_lines.saturating_sub(visible_height);
    }

    app.scroll = app
        .scroll
        .clamp(0, total_lines.saturating_sub(visible_height));

    frame.render_widget(
        paragraph
            .block(Block::bordered().title(format!(" Chatting in Room {} ", app.room)))
            .scroll((app.scroll as u16, 0)),
        messages_area,
    );
}
