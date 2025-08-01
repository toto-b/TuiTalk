use serde::Serialize;
use serde::Deserialize;

#[derive(Serialize, Deserialize, Debug)]
pub struct MyMessage {
    pub username: String,
    pub text: String,
}
