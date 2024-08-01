use std::borrow::Cow;
use std::ops::ControlFlow;
use std::time::Duration;

use futures_util::stream::FuturesUnordered;
use futures_util::{SinkExt, StreamExt};
use tokio::time::Instant;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::protocol::frame::coding::CloseCode;
use tokio_tungstenite::tungstenite::protocol::CloseFrame;
use tokio_tungstenite::tungstenite::Message;

const N_CLIENTS: usize = 10;
const SERVER: &str = "ws://127.0.0.1:3000/ws";

#[tokio::main]
async fn main() {
    let start_time = Instant::now();
    let mut clients = (0..N_CLIENTS)
        .map(|cli| tokio::spawn(spawn_client(cli)))
        .collect::<FuturesUnordered<_>>();

    while clients.next().await.is_some() {}

    let end_time = Instant::now();

    println!(
        "Total time take {:#?} with {N_CLIENTS} concurrent clients, should be about 6.45 seconds",
        end_time - start_time
    );
}

async fn spawn_client(who: usize) {
    let ws_stream = match connect_async(SERVER).await {
        Ok((stream, response)) => {
            println!("Handshake for client {who} has been completed");
            println!("server response was {response:?}");
            stream
        }
        Err(e) => {
            println!("WebSocket handshake for client {who} failed with {e}!");
            return;
        }
    };

    let (mut sender, mut receiver) = ws_stream.split();

    sender
        .send(Message::Ping("Hello, Server!".into()))
        .await
        .expect("Can not send!");

    let mut send_task = tokio::spawn(async move {
        for i in 1..30 {
            if sender
                .send(Message::Text(format!("Message number {i}...")))
                .await
                .is_err()
            {
                return;
            }
            tokio::time::sleep(Duration::from_millis(300)).await;
        }

        println!("Sending close to {who}...");
        if let Err(e) = sender
            .send(Message::Close(Some(CloseFrame {
                code: CloseCode::Normal,
                reason: Cow::from("goodbye"),
            })))
            .await
        {
            println!("Could not send Close due to {e:?}, probably it is ok?");
        }
    });

    let mut recv_task = tokio::spawn(async move {
        while let Some(Ok(msg)) = receiver.next().await {
            if process_message(msg, who).is_break() {
                break;
            }
        }
    });

    tokio::select! {
        _ = (&mut send_task)=>{
            recv_task.abort();
        }
        _ =(&mut recv_task)=>{
            send_task.abort();
        }
    }
}

fn process_message(msg: Message, who: usize) -> ControlFlow<(), ()> {
    match msg {
        Message::Text(t) => {
            println!(">>> {who} got str: {t:?}");
        }
        Message::Binary(d) => {
            println!(">>> {} got {} bytes: {:?}", who, d.len(), d);
        }
        Message::Close(c) => {
            if let Some(cf) = c {
                println!(
                    ">>> {} got close with code {} and reason `{}`",
                    who, cf.code, cf.reason
                );
            } else {
                println!(">>> {who} somehow got close message without CloseFrame");
            }
            return ControlFlow::Break(());
        }
        Message::Pong(v) => {
            println!(">>> {who} got pong with {v:?}");
        }
        Message::Ping(v) => {
            println!(">>> {who} got ping with {v:?}");
        }
        Message::Frame(_) => {
            unreachable!("This is never supposed to happen")
        }
    }
    ControlFlow::Continue(())
}
