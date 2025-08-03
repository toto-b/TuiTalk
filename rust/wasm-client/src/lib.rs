use shared::{ClientAction, TalkProtocol, wasm::WsConnection};
use wasm_bindgen::prelude::*;
use web_sys::{HtmlButtonElement, HtmlElement, HtmlInputElement};
use std::rc::Rc;

#[wasm_bindgen(start)]
pub async fn start() -> Result<(), JsValue> {
    console_error_panic_hook::set_once();
    console_log::init_with_level(log::Level::Debug).unwrap();

    // Create UI elements
    let document = web_sys::window().unwrap().document().unwrap();
    let body = document.body().unwrap();

    // Create input for username
    let username_input: HtmlInputElement = document.create_element("input")?.dyn_into()?;
    username_input.set_attribute("type", "text")?;
    username_input.set_attribute("placeholder", "Username")?;
    body.append_child(&username_input)?;

    // Create input for message
    let message_input: HtmlInputElement = document.create_element("input")?.dyn_into()?;
    message_input.set_attribute("type", "text")?;
    message_input.set_attribute("placeholder", "Message")?;
    body.append_child(&message_input)?;

    // Create send button
    let send_button: HtmlButtonElement = document.create_element("button")?.dyn_into()?;
    send_button.set_text_content(Some("Send"));
    body.append_child(&send_button)?;

    // Create message display area
    let messages_div = document.create_element("div")?.dyn_into::<HtmlElement>()?;
    body.append_child(&messages_div)?;



    let conn = Rc::new(
        WsConnection::connect("ws://localhost:8080", move |msg: TalkProtocol| {
            // Display received messages
            let message_text = format!("{}: {}", msg.username, msg.message);
            let message_element = document.create_element("p").unwrap();
            message_element.set_text_content(Some(&message_text));
            messages_div.append_child(&message_element).unwrap();
        })
        .await
        .unwrap(),
    );

    // Use Rc for the closure
    let conn_clone = Rc::clone(&conn);
    // Clone conn for closure

    // Set up button click handler
    let on_click = Closure::<dyn FnMut()>::new(move || {
        let username = username_input.value();
        let message = message_input.value();

        if !username.is_empty() && !message.is_empty() {
            let msg = TalkProtocol {
                username,
                message,
                action: None, // Or Some(ClientAction::Message)
                room_id: 1,
                unixtime: 100,
            };

            conn_clone.send(msg).unwrap();
            message_input.set_value(""); // Clear input after sending
        }
    });

    send_button.set_onclick(Some(on_click.as_ref().unchecked_ref()));
    on_click.forget(); // Prevent closure from being dropped

    Ok(())
}
