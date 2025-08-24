mod app;
mod ui;
mod command;

use crate::app::App;
use futures_channel::mpsc::unbounded;
use shared::TalkProtocol;
pub use shared::native::{connect, receiver_task, sender_task};
use std::sync::{Arc, Mutex};
use tokio;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let url = std::env::args()
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