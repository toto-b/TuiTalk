use shared::wasm::WsConnection;
use shared::TalkProtocol;
use std::cell::RefCell;
use std::rc::Rc;
use wasm_bindgen_futures::spawn_local;
use yew::prelude::*;

#[function_component]
fn App() -> Html {
    let username = use_state(|| "".to_string());
    let message = use_state(|| "".to_string());

    // magic
    let con = Rc::new(RefCell::new(None));
    let con_clone = Rc::clone(&con);

    spawn_local(async move {
        let connection = WsConnection::connect("ws://localhost:8080").await;
        *con_clone.borrow_mut() = Some(connection);
    });
    // magic end
    let unpack = con.borrow();
    let mut result = (&*unpack).unwrap().unwrap();

    let example_message : TalkProtocol = TalkProtocol { username: "Bing".to_string(), message: "Du dulli".to_string(), action: None, room_id: 1, unixtime: 1 };
    result.ws_sender.unbounded_send(example_message);

    let on_input_username = {
        let username = username.clone();
        Callback::from(move |e: InputEvent| {
            let value = e
                .target_unchecked_into::<web_sys::HtmlInputElement>()
                .value();
            username.set(value);
        })
    };

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
        Callback::from(move |_| {
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
