use crate::app;
use shared::*;
use std::{
    num::ParseIntError,
    time::{SystemTime, UNIX_EPOCH},
};

pub fn get_unix_timestamp() -> u64 {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("unixtime");
    now.as_secs()
}

pub fn join_initial_room(app: &mut app::App) {
    let com = join_room(app);
    app.tx.unbounded_send(com).unwrap();
}

pub fn quit_app(app: &mut app::App) {
    let com = leave_room(app);
    app.tx.unbounded_send(com).unwrap();
}

pub fn join_room(app: &mut app::App) -> TalkProtocol {
    TalkProtocol::JoinRoom {
        room_id: app.room,
        uuid: app.uuid,
        username: app.username.clone(),
        unixtime: get_unix_timestamp(),
    }
}

pub fn leave_room(app: &mut app::App) -> TalkProtocol {
    TalkProtocol::LeaveRoom {
        room_id: app.room,
        uuid: app.uuid,
        username: app.username.clone(),
        unixtime: get_unix_timestamp(),
    }
}

pub fn parse(app: &mut app::App) {
    if app.input.starts_with("/") {
        app.input = app.input.trim_start_matches("/").trim().to_string();
        parse_command(app);
    } else {
        let com = TalkProtocol::PostMessage {
            message: TalkMessage {
                uuid: app.uuid,
                username: app.username.to_string(),
                text: app.input.to_string(),
                room_id: app.room,
                unixtime: get_unix_timestamp(),
            },
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
                app.communication.lock().unwrap().clear();
                app.tx.unbounded_send(com.1).unwrap();
            }
            Err(error) => {
                let com = parse_command_room_invalid(error);
                app.communication.lock().unwrap().push(com);
            }
        }
    } else if app.input == "clear" {
        app.communication.lock().unwrap().clear();
    } else if app.input == "fetch" {
        let com = parse_command_fetch(app, 50);
        app.tx.unbounded_send(com).unwrap();
    } else {
        let com = parse_invalid_command(app);
        app.communication.lock().unwrap().push(com);
    }
}

fn parse_command_room_valid(app: &mut app::App, number: i32) -> (TalkProtocol, TalkProtocol) {
    let leave_message = leave_room(app);
    app.room = number;
    let join_message = join_room(app);
    (leave_message, join_message)
}

fn parse_command_room_invalid(error: ParseIntError) -> TalkProtocol {
    TalkProtocol::LocalError {
        message: error.to_string(),
    }
}

fn parse_command_name(app: &mut app::App) -> TalkProtocol {
    let old_username = app.username.to_string();
    app.username = app.input.to_string();
    TalkProtocol::ChangeName {
        uuid: app.uuid,
        username: app.username.to_string(),
        old_username: old_username.to_string(),
        unixtime: get_unix_timestamp(),
    }
}

fn parse_invalid_command(app: &mut app::App) -> TalkProtocol {
    TalkProtocol::LocalError {
        message: format!("The command '{}' does not exist", app.input),
    }
}

fn parse_command_fetch(app: &mut app::App, set_limit: i64) -> TalkProtocol {
    TalkProtocol::Fetch {
        room_id: app.room,
        limit: set_limit,
        fetch_before: app
            .communication
            .lock()
            .unwrap()
            .iter()
            .find_map(|proto| match proto {
                TalkProtocol::Error { .. } => None,
                TalkProtocol::LocalError { .. } => None,
                TalkProtocol::PostMessage { message } => Some(message.unixtime),
                TalkProtocol::UserJoined { unixtime, .. } => Some(*unixtime),
                TalkProtocol::UserLeft { unixtime, .. } => Some(*unixtime),
                TalkProtocol::UsernameChanged { unixtime, .. } => Some(*unixtime),
                _ => None,
            })
            .unwrap_or(get_unix_timestamp()),
    }
}
