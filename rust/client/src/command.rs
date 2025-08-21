use crate::app;
use shared::{ClientAction::Send, TalkProtocol};
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

fn get_unix_timestamp() -> u64 {
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
    let mut com = TalkProtocol {
        uuid: Uuid::new_v4(),
        username: app.username.clone(),
        message: Some("".to_string()),
        action: Send,
        room_id: app.room,
        unixtime: get_unix_timestamp(),
    };
    if app.input.starts_with("name") {
        app.input = app.input.trim_start_matches("name ").trim().to_string();
        app.username = app.input.to_string();
        let message = format!("changed his name to '{}'", app.username);
        com.message = Some(message.to_string());
        app.tx.unbounded_send(com).unwrap();
    } else if app.input.starts_with("room") {
        app.input = app.input.trim_start_matches("room").trim().to_string();
        match app.input.parse::<i32>() {
            Ok(number) => {
                app.room = number;
                let message = format!("changed to room {}", app.room);
                com.message = Some(message.to_string());
                app.tx.unbounded_send(com).unwrap();
            }
            Err(error) => {
                com.username = "Error".to_string();
                com.message = Some(error.to_string());
                app.communication.lock().unwrap().push(com);
            }
        }
    } else if app.input.starts_with("broadcast") {
        app.input = app.input.trim_start_matches("broadcast").trim().to_string();
        com.username = "Broadcast".to_string();
        com.message = Some(app.input.to_string());
        app.tx.unbounded_send(com).unwrap();
    } else {
        com.username = "Error".to_string();
        let message = format!("The command '{}' does not exist", app.input);
        com.message = Some(message.to_string());
        app.communication.lock().unwrap().push(com);
    }
}
