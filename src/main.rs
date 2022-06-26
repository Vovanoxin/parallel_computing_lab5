use std::convert::Infallible;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use futures::SinkExt;
use hyper::{header, upgrade, StatusCode, Body, Request, Response, Server, server::conn::AddrStream};
use hyper::service::{make_service_fn, service_fn};
use tokio_tungstenite::WebSocketStream;
use tungstenite::{handshake, Error, Message};
use futures::stream::StreamExt;
use hyper::upgrade::Upgraded;

struct WebSocketClient {
    request: Request<Body>,
    remote_addr: SocketAddr,
}

impl WebSocketClient {
    fn new(request: Request<Body>, remote_addr: SocketAddr) -> WebSocketClient {
        WebSocketClient {
            request,
            remote_addr
        }
    }

    async fn handle_messages(self, upgraded: Upgraded) {
        println!("waiting for stream");
        let ws_stream = WebSocketStream::from_raw_socket(
            upgraded,
            tungstenite::protocol::Role::Server,
            None,
        ).await;
        println!("waiting for msgs");
        let (mut ws_write, mut ws_read) = ws_stream.split();

        while let Some(msg) = ws_read.next().await {
            println!("Got msg: {:?}", msg);
            match msg {
                Ok(msg) => {
                    ws_write.send(Message::Text("Hello from server".to_owned())).await.unwrap();
                }
                Err(Error::ConnectionClosed) => println!("Connection closed normally"),
                Err(e) =>
                    println!("error creating echo stream. Error is {}", e),
            }
        }
    }
}

struct WebSocketServer {
    clients: Arc<Mutex<Vec<WebSocketClient>>>,
}

impl WebSocketServer {

}



fn handle_ws_connection(mut ws_client: WebSocketClient) -> Result<Response<Body>, Infallible> {
    let response =
        match handshake::server::create_response_with_body(&ws_client.request, || Body::empty()) {
            Ok(response) => {
                tokio::spawn(async move {
                    match upgrade::on(&mut ws_client.request).await {
                        Ok(upgraded) => {
                            println!("Connection upgraded");
                            ws_client.handle_messages(upgraded).await;
                        },
                        Err(e) =>
                            println!("error when trying to upgrade connection. \
                                        Error is: {}", e),
                    }
                });
                response
            },
            Err(error) => {
                println!("Failed to create websocket response \
                                Error is: {}", error);
                let mut res = Response::new(Body::from(format!("Failed to create websocket: {}", error)));
                *res.status_mut() = StatusCode::BAD_REQUEST;
                return Ok(res);
            }
        };

    Ok::<_, Infallible>(response)
}

async fn handle_request(mut request: Request<Body>,
                        remote_addr: SocketAddr,
                        arc: Arc<SharedState>) -> Result<Response<Body>, Infallible> {
    match (request.uri().path(), request.headers().contains_key(header::UPGRADE)) {
        ("/ws_echo", true) => {
            println!("Handling ws request");
            handle_ws_connection(WebSocketClient::new(request, remote_addr))

        },
        (url@_, false) => {
            Ok(Response::new(Body::default()))
        },
        (_, true) => {
            Ok(Response::new(Body::default()))
        }
    }
}

struct SharedState {
    ws_clients: Arc<Mutex<Vec<WebSocketClient>>>
}

#[tokio::main]
async fn main() {
    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    println!("Listening on {} for http or websocket connections.", addr);

    let shared_state = Arc::new(SharedState{ws_clients:Arc::new(Mutex::new(Vec::new()))});

    let make_svc = make_service_fn(|conn: & AddrStream| {
        let remote_addr = conn.remote_addr();
        let shared_state = shared_state.clone();
        async move {
            // service_fn converts our function into a `Service`
            Ok::<_, Infallible>(service_fn(move |request: Request<Body>|
                handle_request(request, remote_addr, shared_state.clone())
            ))
        }
    });

    let server = Server::bind(&addr).serve(make_svc);

    // Run this server for... forever!
    if let Err(e) = server.await {
        eprintln!("server error: {}", e);
    }
}