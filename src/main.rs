extern crate pretty_env_logger;
#[macro_use] extern crate log;

use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::Mutex;
use futures::{SinkExt, StreamExt};
use tokio::net::{TcpListener, TcpStream};
use tokio_tungstenite::{accept_async};
use tokio::sync::broadcast;
use tokio::sync::broadcast::{Sender, Receiver};
use tokio::{select};
use std::fs;
use std::ops::Deref;
use serde::{Serialize, Deserialize};

#[derive(Copy, Clone)]
enum BroadcastMSG {
    LabsChanged,
    NoMoreLabs,
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "command")]
enum WebSockJSONMsg {
    getLabs,
    takeLab{id: usize},
}

async fn handle_connection(peer: SocketAddr,
                           stream: TcpStream,
                           labs: Arc<Mutex<Vec<Lab>>>,
                           tx: Sender<BroadcastMSG>,
                           mut rx: Receiver<BroadcastMSG>) -> tungstenite::Result<()> {
    let mut ws_stream = accept_async(stream).await.expect("Failed to accept");

    info!("New WebSocket connection: {}", peer);

    loop {
        select! {
            maybe_msg = ws_stream.next() => {
                if let Some(msg) = maybe_msg {
                    let msg = msg?;
                    info!("got msg: {:?}", msg);
                    let json_msg = serde_json::from_str(msg.to_text().unwrap_or(""));
                    match json_msg {
                        Ok(WebSockJSONMsg::getLabs) => {
                            info!("Processing getLabs");
                            let labs = labs.lock().await;
                            info!("Labs locked!");
                            ws_stream.send(tungstenite::Message::Text(
                                serde_json::to_string(
                                    &labs.deref()
                                    .into_iter()
                                    .filter(|lab| !lab.taken)
                                    .collect::<Vec<_>>()).unwrap()
                            )).await;
                        },
                        Ok(WebSockJSONMsg::takeLab{id}) => {
                            info!("Processing takeLab");
                            {
                                let mut labs = labs.lock().await;
                                if id > labs.len() || labs[id].taken {
                                    ws_stream.send(tungstenite::Message::Text(String::from("{\"info\":\"fail\"}"))).await;
                                    continue;
                                }
                                labs[id].taken = true;
                                if labs.deref().iter().all(|lab| lab.taken) {
                                    tx.send(BroadcastMSG::NoMoreLabs);
                                }
                            }
                            ws_stream.send(tungstenite::Message::Text(String::from("{\"info\":\"success\"}"))).await;

                            tx.send(BroadcastMSG::LabsChanged);
                        },
                        _ => {
                            ws_stream.send(tungstenite::Message::Text(String::from("{\"info\":\"unknown\"}"))).await;
                            info!("Got msg: {}", msg);
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
                        ws_stream.send(tungstenite::Message::Text(String::from("{\"info\":\"labsChanged\"}"))).await;
                        info!("Got LabsChanged");
                    },
                    BroadcastMSG::NoMoreLabs => break,
                };
            }
        }
    }
    info!("Release connection {}", peer);
    Ok(())
}

#[derive(Serialize, Deserialize, Debug)]
struct Lab {
    #[serde(skip_deserializing)]
    id: usize,
    title: String,
    description: String,
    #[serde(skip_deserializing,skip_serializing)]
    taken: bool,
}

#[tokio::main]
async fn main() {
    pretty_env_logger::init();
    let labs_data = fs::read_to_string("labs.json").expect("Unable to read labs.json");
    let mut labs_vec: Vec<Lab> = serde_json::from_str(&labs_data).unwrap();
    for i in 0..labs_vec.len() {
        labs_vec[i].id = i;
    }
    let labs: Arc<Mutex<Vec<Lab>>> = Arc::new(Mutex::new(labs_vec));

    let addr = "127.0.0.1:3000";
    let listener = TcpListener::bind(&addr).await.expect("Can't listen");
    info!("Listening on: {}", addr);

    let (tx, mut rx) = broadcast::channel(512);
    loop {
        select! {
            maybe_stream = listener.accept() => {
                if let Ok((stream, _)) = maybe_stream {
                    let peer = stream.peer_addr().expect("connected streams should have a peer address");
                    info!("Peer address: {}", peer);
                    tokio::spawn(handle_connection(peer, stream, labs.clone(), tx.clone(), tx.subscribe()));
                }
            }
            maybe_broadcast_msg = rx.recv() => {
                let broadcast_msg: BroadcastMSG = maybe_broadcast_msg.unwrap();
                match broadcast_msg {
                    BroadcastMSG::NoMoreLabs => break,
                    _ => (),
                };
            }

        }
    }
}