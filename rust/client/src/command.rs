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

pub fn parse(app: &mut app::App) -> TalkProtocol {
    if app.input.starts_with("/name") && app.input.len() <= 20 {
        let name = app.input.trim_start_matches("/name ").trim();
        let old_name = app.username.to_string();
        app.username = name.to_string();
        let msg = format!("changed his username to {}", app.username);
        let com = TalkProtocol {
            uuid: Uuid::new_v4(),
            username: old_name.to_string(),
            message: Some(msg),
            action: Send,
            room_id: app.room,
            unixtime: get_unix_timestamp(),
        };
        com
    } else {
        let com = TalkProtocol {
            uuid: Uuid::new_v4(),
            username: app.username.to_string(),
            message: Some(app.input.to_string()),
            action: Send,
            room_id: app.room,
            unixtime: get_unix_timestamp(),
        };
        com
    }
}
