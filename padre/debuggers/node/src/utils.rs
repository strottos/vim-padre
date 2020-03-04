use tokio_tungstenite::tungstenite::protocol::Message;

pub fn get_message_id(json: &serde_json::Value) -> Option<u64> {
    match json.get("id") {
        Some(x) => x.as_u64(),
        None => None,
    }
}

pub fn get_json(message: &Message) -> serde_json::Value {
    if let Message::Text(s) = &message {
        serde_json::from_str(&s).unwrap()
    } else {
        panic!("Can't understand message: {:?}", message);
    }
}
