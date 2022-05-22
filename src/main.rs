use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr};
use tokio::io::{AsyncRead, AsyncReadExt};
use tokio::net::{TcpListener, TcpStream};

#[tokio::main]
async fn main() {
    let listener = TcpListener::bind("127.0.0.1:6379").await.unwrap();
    let mut clients: HashMap<u32, TcpStream>;
    loop {
        let (mut stream, _) = listener.accept().await.unwrap();
        process(stream).await;
    }
}

async fn process(mut stream: TcpStream) {
    let mut buf = [0; 1024];
    stream.read(&mut buf).await.unwrap();
    println!("{:?}", buf);
}
