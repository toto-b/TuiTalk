pub mod wasm_client;

use wasm_client::ChatClient;

fn main() {
    console_error_panic_hook::set_once();
    console_log::init_with_level(log::Level::Info).expect("Failed to initialize logger");
    
    yew::Renderer::<ChatClient>::new().render();
}
