//! Websocket connection

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use super::analyser::Analyser;
use super::utils::{get_json, get_message_id};

use futures::{SinkExt, StreamExt};
use tokio::sync::{mpsc, oneshot};
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::protocol::Message;
use url::Url;

#[derive(Debug)]
pub struct WSHandler {
    response_listeners: Arc<Mutex<HashMap<u64, oneshot::Sender<Message>>>>,
    tx: mpsc::Sender<Message>,
    ws_id: u64,
}

impl WSHandler {
    pub fn new(
        uri: &str,
        analyser: Arc<Mutex<Analyser>>,
        node_tx: mpsc::Sender<(Message, Option<oneshot::Sender<Message>>)>,
    ) -> Self {
        let (tx, mut rx) = mpsc::channel(1);

        let response_listeners: Arc<Mutex<HashMap<u64, oneshot::Sender<Message>>>> =
            Arc::new(Mutex::new(HashMap::new()));

        let url = Url::parse(uri).unwrap();

        let response_listeners_moved = response_listeners.clone();

        tokio::spawn(async move {
            let (ws_stream, _) = connect_async(url)
                .await
                .expect("Can't connect to Websocket");

            let (mut ws_write, mut ws_read) = ws_stream.split();

            let analyser = analyser.clone();

            tokio::spawn(async move {
                let analyser = analyser.clone();
                while let Some(Ok(message)) = ws_read.next().await {
                    match message {
                        Message::Text(_) => {
                            let json = get_json(&message);
                            let id = get_message_id(&json);

                            match id {
                                Some(id) => {
                                    match response_listeners_moved.lock().unwrap().remove(&id) {
                                        Some(listener_tx) => {
                                            listener_tx.send(message).unwrap();
                                        }
                                        None => {}
                                    }
                                }
                                None => {
                                    analyser
                                        .lock()
                                        .unwrap()
                                        .analyse_message(json, node_tx.clone());
                                }
                            };
                        }
                        _ => {}
                    };
                }
            });

            while let Some(message) = rx.next().await {
                ws_write.send(message).await.unwrap();
            }
        });

        WSHandler {
            response_listeners,
            tx: tx,
            ws_id: 1,
        }
    }

    pub async fn send_and_receive_message(&mut self, mut message: Message) -> Message {
        let (listener_tx, listener_rx) = oneshot::channel();

        match message {
            Message::Text(_) => {
                let json = get_json(&message);
                let id = get_message_id(&json);

                let id = match id {
                    Some(x) => x,
                    None => {
                        let next_id = self.get_next_ws_id();
                        message = self.add_id_to_message(message, next_id);
                        let json = get_json(&message);
                        get_message_id(&json).unwrap()
                    }
                };

                self.response_listeners
                    .lock()
                    .unwrap()
                    .insert(id, listener_tx);
            }
            _ => {}
        };

        self.tx.send(message).await.unwrap();

        listener_rx.await.unwrap()
    }

    pub fn add_id_to_message(&self, message: Message, id: u64) -> Message {
        if let Message::Text(s) = message {
            let mut json: serde_json::Value = serde_json::from_str(&s).unwrap();
            json["id"] = serde_json::json!(id);
            Message::Text(json.to_string())
        } else {
            unreachable!();
        }
    }

    fn get_next_ws_id(&mut self) -> u64 {
        let id = self.ws_id;
        self.ws_id += 1;
        id
    }
}
