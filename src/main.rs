extern crate pretty_env_logger;
#[macro_use] extern crate log;

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use futures::{SinkExt, StreamExt};
use tokio::net::{TcpListener, TcpStream};
use tokio_tungstenite::{accept_async, tungstenite::Error};
use tungstenite::{Message, Result};
use tokio::sync::broadcast;
use tokio::sync::broadcast::{Sender, Receiver};
use tokio::{select};
use std::fs;
use serde::{Serialize, Deserialize};

#[derive(Copy, Clone)]
enum BroadcastMSG {
    LabsChanged,
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "command")]
enum WebSockJSONMsg {
    getLabs,
    takeLab{id: usize},
}


async fn accept_connection(peer: SocketAddr, stream: TcpStream, labs: Arc<Mutex<Vec<Lab>>>, tx: Sender<BroadcastMSG>, mut rx: Receiver<BroadcastMSG>) {
    if let Err(e) = handle_connection(peer, stream, labs, tx, rx).await {
        match e {
            Error::ConnectionClosed | Error::Protocol(_) | Error::Utf8 => (),
            err => error!("Error processing connection: {}", err),
        }
    }
}

async fn handle_connection(peer: SocketAddr, stream: TcpStream, labs: Arc<Mutex<Vec<Lab>>>, tx: Sender<BroadcastMSG>, mut rx: Receiver<BroadcastMSG>) -> Result<()> {
    let mut ws_stream = accept_async(stream).await.expect("Failed to accept");

    info!("New WebSocket connection: {}", peer);

    loop {
        select! {
            maybe_msg = ws_stream.next() => {
                if let Some(msg) = maybe_msg {
                    let msg = msg?;
                    match msg.to_text().unwrap_or`("") {
                        _ => {
                            ws_stream.send(tungstenite::Message::Text(format!("Echo msg: {}", msg))).await;
                            info!("Got msg: {}", msg);
                            tx.send(BroadcastMSG::LabsChanged);
                            continue;
                        }
                    }
                } else {
                    break;
                }
            }

            maybe_broadcast_msg = rx.recv() => {
                let broadcast_msg: BroadcastMSG = maybe_broadcast_msg.unwrap();
                match broadcast_msg {
                    BroadcastMSG::LabsChanged => {
                        ws_stream.send(tungstenite::Message::Text(format!("Got lab changed"))).await;
                        info!("Got LabsChanged");
                    }
                };
            }
        }
    }

    Ok(())
}

struct SharedData {
    labs: Arc<Mutex<Vec<Lab>>>,
}

#[derive(Serialize, Deserialize, Debug)]
struct Lab {
    title: String,
    description: String,
    #[serde(skip_deserializing,skip_serializing)]
    taken: bool,
}

#[tokio::main]
async fn main() {
    pretty_env_logger::init();

    // read labs from labs.json
    let labs_data = fs::read_to_string("labs.json").expect("Unable to read labs.json");
    let labs: Arc<Mutex<Vec<Lab>>> = Arc::new(Mutex::new(serde_json::from_str(&labs_data).unwrap()));

    let addr = "127.0.0.1:3000";
    let listener = TcpListener::bind(&addr).await.expect("Can't listen");
    info!("Listening on: {}", addr);

    let (tx, _) = broadcast::channel(512);

    while let Ok((stream, _)) = listener.accept().await {
        let peer = stream.peer_addr().expect("connected streams should have a peer address");
        info!("Peer address: {}", peer);

        tokio::spawn(accept_connection(peer, stream, labs.clone(), tx.clone(), tx.subscribe()));
    }
}