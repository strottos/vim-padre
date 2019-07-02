//! Websocket connection

use std::collections::HashMap;
use std::io;
use std::sync::{Arc, Mutex};

use crate::notifier::{LogLevel, Notifier};

use tokio::prelude::*;
use tokio::sync::mpsc::{self, Sender};
use websocket::result::WebSocketError;
use websocket::{ClientBuilder, OwnedMessage};

#[derive(Debug)]
pub struct WSHandler {
    notifier: Arc<Mutex<Notifier>>,
    response_listeners: Arc<Mutex<HashMap<u64, Sender<serde_json::Value>>>>,
    ws_tx: Option<Sender<OwnedMessage>>,
    ws_id: u64,
}

impl WSHandler {
    pub fn new(notifier: Arc<Mutex<Notifier>>) -> WSHandler {
        WSHandler {
            notifier,
            response_listeners: Arc::new(Mutex::new(HashMap::new())),
            ws_tx: None,
            ws_id: 1,
        }
    }

    pub fn connect<F>(&mut self, uri: &str, f: F)
    where
        F: Fn(serde_json::Value) -> Option<OwnedMessage> + Sync + Send + 'static,
    {
        let (tx, rx) = mpsc::channel(1);

        self.ws_tx = Some(tx.clone());
        let response_listeners = self.response_listeners.clone();
        let notifier = self.notifier.clone();

        let fut = ClientBuilder::new(uri)
            .unwrap()
            .async_connect_insecure()
            .and_then(move |(duplex, _)| {
                let (sink, stream) = duplex.split();

                stream
                    .filter_map(move |message| {
                        let json: serde_json::Value;
                        if let OwnedMessage::Text(s) = &message {
                            json = serde_json::from_str(s).unwrap();
                        } else if message.is_close() {
                            return Some(OwnedMessage::Close(None));
                        } else {
                            panic!("Can't understand message: {:?}", message)
                        }

                        if json["method"].is_string() {
                            f(json);
                        } else if json["id"].is_number() {
                            let id = json["id"].clone();
                            let id: u64 = match serde_json::from_value(id) {
                                Ok(s) => s,
                                Err(e) => {
                                    panic!("Can't understand id: {:?}", e);
                                }
                            };

                            match response_listeners.lock().unwrap().remove(&id) {
                                Some(listener_tx) => {
                                    tokio::spawn(listener_tx.send(json).map(|_| {}).map_err(|e| {
                                        eprintln!("Error spawning node: {:?}", e);
                                    }));
                                }
                                None => {}
                            };
                        } else {
                            notifier
                                .lock()
                                .unwrap()
                                .log_msg(LogLevel::ERROR, format!("Response error: {}", json));
                        };
                        None
                    })
                    .select(rx.map_err(|_| WebSocketError::NoDataAvailable))
                    .forward(sink)
            })
            .map(|_| ())
            .map_err(|e| eprintln!("WebSocket err: {:?}", e));

        tokio::spawn(fut);
    }

    fn get_next_ws_id(&mut self) -> u64 {
        let id = self.ws_id;
        self.ws_id += 1;
        id
    }

    fn add_id_to_message(&self, msg: OwnedMessage, id: u64) -> OwnedMessage {
        if let OwnedMessage::Text(s) = &msg {
            let mut json: serde_json::Value = serde_json::from_str(s).unwrap();
            json["id"] = serde_json::json!(id);
            OwnedMessage::Text(json.to_string())
        } else {
            unreachable!();
        }
    }

    pub fn close(&self) {
        let tx = self.ws_tx.clone();

        tokio::spawn(tx.unwrap().send(OwnedMessage::Close(None)).map(|_| {}).map_err(|e| {
            eprintln!("Error sending message: {:?}", e);
        }));
    }

    pub fn send_and_receive_message(
        &mut self,
        msg: OwnedMessage,
    ) -> Box<dyn Future<Item = serde_json::Value, Error = io::Error> + Send> {
        let id = self.get_next_ws_id();
        let msg = self.add_id_to_message(msg, id);

        let (listener_tx, listener_rx) = mpsc::channel(1);

        self.response_listeners
            .lock()
            .unwrap()
            .insert(id, listener_tx);

        let tx = self.ws_tx.clone();

        tokio::spawn(tx.unwrap().send(msg).map(|_| {}).map_err(|e| {
            eprintln!("Error sending message: {:?}", e);
        }));

        let f = listener_rx
            .into_future()
            .map(move |response| response.0.unwrap())
            .map_err(|e| {
                eprintln!("Error sending to node: {:?}", e.0);
                io::Error::new(io::ErrorKind::Other, "Timed out sending to node")
            });

        Box::new(f)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};
    use websocket::OwnedMessage;

    fn notifier() -> Arc<Mutex<crate::notifier::Notifier>> {
        Arc::new(Mutex::new(crate::notifier::Notifier::new()))
    }

    #[test]
    fn check_add_message_id() {
        let mut ws_handler = super::WSHandler::new(notifier());

        let msg = OwnedMessage::Text("{\"TEST\":1}".to_string());
        let id = ws_handler.get_next_ws_id();
        let msg = ws_handler.add_id_to_message(msg, id);
        let json: serde_json::Value;
        if let OwnedMessage::Text(s) = msg {
            json = serde_json::from_str(&s).unwrap();
        } else {
            unreachable!();
        }

        let expected = "{\"id\":1,\"TEST\":1}";
        let expected: serde_json::Value = serde_json::from_str(expected).unwrap();

        assert_eq!(expected, json);
        assert_eq!(2, ws_handler.ws_id);
    }
}
