use crate::app;
use shared::*;
use std::{
    num::ParseIntError,
    time::{SystemTime, UNIX_EPOCH},
};

const MESSAGE_LENGTH: usize = 250;
const USERNAME_LENGTH: usize = 15;

pub fn get_unix_timestamp() -> u64 {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("unixtime");
    now.as_secs()
}

pub fn get_first_message_timestamp(app: &mut app::App) -> u64 {
    app.communication
        .lock()
        .expect("Vector of communication")
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
        .unwrap_or(get_unix_timestamp())
}

pub fn join_initial_room(app: &mut app::App) {
    let com = join_room(app);
    app.tx
        .unbounded_send(com)
        .expect("Successfully transmitted");
}

pub fn quit_app(app: &mut app::App) {
    let com = leave_room(app);
    app.tx
        .unbounded_send(com)
        .expect("Successfully transmitted");
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
    if app.input.is_empty() {
    } else if app.input.len() >= MESSAGE_LENGTH {
        let com = parse_message_too_long();
        app.tx
            .unbounded_send(com)
            .expect("Successfully transmitted");
    } else if app.input.starts_with("/") {
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
        app.tx
            .unbounded_send(com)
            .expect("Successfully transmitted");
    }
}

fn parse_command(app: &mut app::App) {
    if app.input.starts_with("name") {
        app.input = app.input.trim_start_matches("name ").trim().to_string();
        if app.input.len() <= USERNAME_LENGTH {
            let com = parse_command_name(app);
            app.tx
                .unbounded_send(com)
                .expect("Successfully transmitted");
        } else {
            let com = parse_message_too_long();
            app.communication
                .lock()
                .expect("Communication Vector")
                .push(com);
        }
    } else if app.input.starts_with("room") {
        app.input = app.input.trim_start_matches("room").trim().to_string();
        match app.input.parse::<i32>() {
            Ok(number) => {
                let (leave, join) = parse_command_room_valid(app, number);
                app.tx
                    .unbounded_send(join)
                    .expect("Successfully transmitted");
                app.communication
                    .lock()
                    .expect("Communication Vector")
                    .clear();
                app.tx
                    .unbounded_send(leave)
                    .expect("Successfully transmitted");
            }
            Err(error) => {
                let com = parse_command_room_invalid(error);
                app.communication
                    .lock()
                    .expect("Communication Vector")
                    .push(com);
            }
        }
    } else if app.input == "clear" {
        app.communication
            .lock()
            .expect("Communication Vector")
            .clear();
    } else if app.input.starts_with("fetch") {
        app.input = app.input.trim_start_matches("fetch").trim().to_string();
        match app.input.parse::<i64>() {
            Ok(number) => {
                let com = parse_command_fetch_valid(app, number);
                app.tx
                    .unbounded_send(com)
                    .expect("Successfully transmitted");
            }
            Err(error) => {
                let com = parse_command_fetch_invalid(error);
                app.communication
                    .lock()
                    .expect("Communication Vector")
                    .push(com);
            }
        }
    } else {
        let com = parse_invalid_command(app);
        app.communication
            .lock()
            .expect("Communication Vector")
            .push(com);
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

fn parse_command_fetch_valid(app: &mut app::App, set_limit: i64) -> TalkProtocol {
    TalkProtocol::Fetch {
        room_id: app.room,
        limit: set_limit,
        fetch_before: get_first_message_timestamp(app),
    }
}

fn parse_command_fetch_invalid(error: ParseIntError) -> TalkProtocol {
    TalkProtocol::LocalError {
        message: error.to_string(),
    }
}

fn parse_message_too_long() -> TalkProtocol {
    TalkProtocol::LocalError {
        message: "Input too long".to_string(),
    }
}
