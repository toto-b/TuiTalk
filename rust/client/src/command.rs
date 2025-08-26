use crate::app;
use crossterm::terminal::disable_raw_mode;
use shared::{ClientAction::*, TalkProtocol};
use std::{
    num::ParseIntError,
    process,
    time::{SystemTime, UNIX_EPOCH},
};
use uuid::Uuid;

pub fn get_unix_timestamp() -> u64 {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("unixtime");
    now.as_secs()
}

pub fn parse(app: &mut app::App) {
    if app.input.starts_with("/") {
        app.input = app.input.trim_start_matches("/").trim().to_string();
        parse_command(app);
    } else {
        let com = TalkProtocol {
            uuid: Uuid::new_v4(),
            username: app.username.to_string(),
            message: Some(app.input.to_string()),
            action: Send,
            room_id: app.room,
            unixtime: get_unix_timestamp(),
        };
        app.tx.unbounded_send(com).unwrap();
    }
}

fn parse_command(app: &mut app::App) {
    if app.input.starts_with("name") {
        app.input = app.input.trim_start_matches("name ").trim().to_string();
        let com = parse_command_name(app);
        app.tx.unbounded_send(com).unwrap();
    } else if app.input.starts_with("room") {
        app.input = app.input.trim_start_matches("room").trim().to_string();
        match app.input.parse::<i32>() {
            Ok(number) => {
                let com = parse_command_room_valid(app, number);
                app.tx.unbounded_send(com.0).unwrap();
                app.tx.unbounded_send(com.1).unwrap();
            }
            Err(error) => {
                let com = parse_command_room_invalid(app, error);
                app.communication.lock().unwrap().push(com);
            }
        }
    } else if app.input.starts_with("broadcast") {
        app.input = app.input.trim_start_matches("broadcast").trim().to_string();
        let com = parse_command_broadcast(app);
        app.tx.unbounded_send(com).unwrap();
    } else if app.input == "clear" {
        app.communication.lock().unwrap().clear();
    } else if app.input == "fetch" {
        let com = parse_command_fetch(app);
        app.tx.unbounded_send(com).unwrap();
    } else {
        let com = parse_invalid_command(app);
        app.communication.lock().unwrap().push(com);
    }
}

fn parse_command_room_valid(app: &mut app::App, number: i32) -> (TalkProtocol, TalkProtocol) {
    let leave_message = TalkProtocol {
        uuid: Uuid::new_v4(),
        username: "Info".to_string(),
        message: Some(format!("{} changed to room {}", app.username, number)),
        action: Leave,
        room_id: app.room,
        unixtime: get_unix_timestamp(),
    };
    app.room = number;
    let join_message = TalkProtocol {
        uuid: Uuid::new_v4(),
        username: "Info".to_string(),
        message: Some(format!("{} joined the room", app.username)),
        action: Join,
        room_id: app.room,
        unixtime: get_unix_timestamp(),
    };
    (leave_message, join_message)
}

fn parse_command_room_invalid(app: &mut app::App, error: ParseIntError) -> TalkProtocol {
    TalkProtocol {
        uuid: Uuid::new_v4(),
        username: "Error".to_string(),
        message: Some(error.to_string()),
        action: Send,
        room_id: app.room,
        unixtime: get_unix_timestamp(),
    }
}

fn parse_command_name(app: &mut app::App) -> TalkProtocol {
    let old_username = app.username.to_string();
    app.username = app.input.to_string();
    TalkProtocol {
        uuid: Uuid::new_v4(),
        username: "Info".to_string(),
        message: Some(format!(
            "{} changed his name to '{}'",
            old_username, app.username
        )),
        action: Send,
        room_id: app.room,
        unixtime: get_unix_timestamp(),
    }
}

fn parse_invalid_command(app: &mut app::App) -> TalkProtocol {
    TalkProtocol {
        uuid: Uuid::new_v4(),
        username: "Error".to_string(),
        message: Some(format!("The command '{}' does not exist", app.input)),
        action: Send,
        room_id: app.room,
        unixtime: get_unix_timestamp(),
    }
}

fn parse_command_broadcast(app: &mut app::App) -> TalkProtocol {
    TalkProtocol {
        uuid: Uuid::new_v4(),
        username: "Broadcast".to_string(),
        message: Some(app.input.to_string()),
        action: Send,
        room_id: app.room,
        unixtime: get_unix_timestamp(),
    }
}

fn parse_command_fetch(app: &mut app::App) -> TalkProtocol {
    TalkProtocol {
        uuid: Uuid::new_v4(),
        username: "Info".to_string(),
        message: Some("Fetch requested".to_string()),
        action: Fetch,
        room_id: app.room,
        unixtime: get_unix_timestamp(),
    }
}
