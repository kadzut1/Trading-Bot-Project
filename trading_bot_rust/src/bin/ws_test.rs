use futures_util::{SinkExt, StreamExt};
use serde_json::json;
use std::env;
use tokio_tungstenite::{connect_async, tungstenite::Message};

#[tokio::main]
async fn main() {
    let token = env::var("TINVEST_TOKEN").expect("TINVEST_TOKEN not set");
    let figi = "BBG004730N88";
    let url = "wss://sandbox-invest-public-api.tbank.ru/ws";

    println!("🔌 Connecting to {}", url);

    match connect_async(url).await {
        Ok((ws_stream, _)) => {
            println!("✅ Connected");

            let (mut sink, mut stream) = ws_stream.split();

            // Аутентификация
            let auth = json!({
                "event": "auth",
                "token": token
            });
            sink.send(Message::Text(auth.to_string().into()))
                .await
                .unwrap();
            println!("🔑 Auth sent");

            tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;

            // Подписка на свечи
            let sub_candles = json!({
                "event": "subscribe",
                "figi": figi,
                "interval": "1min"
            });
            sink.send(Message::Text(sub_candles.to_string().into()))
                .await
                .unwrap();
            println!("📊 Subscribed to candles");

            // Подписка на стакан
            let sub_orderbook = json!({
                "event": "subscribe",
                "figi": figi,
                "depth": 10,
                "type": "orderbook"
            });
            sink.send(Message::Text(sub_orderbook.to_string().into()))
                .await
                .unwrap();
            println!("📚 Subscribed to orderbook");

            // Чтение сообщений
            while let Some(msg) = stream.next().await {
                match msg {
                    Ok(Message::Text(text)) => {
                        println!("📩 {}", &text[..text.len().min(300)]);
                    }
                    Ok(Message::Close(_)) => {
                        println!("🔌 Connection closed");
                        break;
                    }
                    Err(e) => {
                        println!("❌ Error: {}", e);
                        break;
                    }
                    _ => {}
                }
            }
        }
        Err(e) => {
            println!("❌ Failed to connect: {}", e);
        }
    }
}
