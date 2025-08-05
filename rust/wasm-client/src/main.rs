use yew::prelude::*;
use shared::wasm::WsConnection;
use wasm_bindgen_futures::spawn_local;
use std::rc::Rc;
use std::cell::RefCell;

#[function_component]
fn App() -> Html {
    let username = use_state(|| "".to_string());
    let message = use_state(|| "".to_string());

    let on_input_username = {
        let username = username.clone();
        Callback::from(move |e: InputEvent| {
            let value = e
                .target_unchecked_into::<web_sys::HtmlInputElement>()
                .value();
            username.set(value);
        })
    };

    // Assuming WsConnection is your WebSocket type
    let con = Rc::new(RefCell::new(None)); // Create a shareable, mutable reference
    // Clone the Rc for the closure
    let con_clone = Rc::clone(&con);

    spawn_local(async move {
        let connection = WsConnection::connect("ws://localhost:8080").await;

        // Store the connection in our shared reference
        *con_clone.borrow_mut() = Some(connection);
    });


    let on_input_message = {
        let message = message.clone();
        Callback::from(move |e: InputEvent| {
            let value = e
                .target_unchecked_into::<web_sys::HtmlInputElement>()
                .value();
            message.set(value);
        })
    };

    let on_send = {
        let username = username.clone();
        let message = message.clone();

        Callback::from(move |_| {
            username.set((*username).clone());
            message.set((*message).clone());
        })
    };

    html! {
        <div>
            <input type="text" oninput={on_input_username} />
            <input type="text" oninput={on_input_message} />
            <button onclick={on_send}>
                { "Click me!" }
            </button>
            <p>{ format!("Username: {}", *username) }</p>
            <p>{ format!("Message: {}", *message) }</p>
        </div>
        }
}

fn main() {
    yew::Renderer::<App>::new().render();
}
