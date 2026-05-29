use tokio_tungstenite::{connect_async, tungstenite::Message};
use futures_util::{SinkExt, StreamExt};

#[tokio::main]
async fn main() {
    let token = std::env::var("TINVEST_TOKEN").expect("TINVEST_TOKEN not set");
    let figi = "BBG004730N88";
    
    let url = "wss://sandbox-invest-public-api.tbank.ru/ws";
    
    println!("🔌 Connecting to {}", url);
    println!("🔑 Token: {}...", &token[..20]);
    
    match connect_async(url).await {
        Ok((ws_stream, _)) => {
            println!("✅ WebSocket connected");
            
            let (mut sink, mut stream) = ws_stream.split();
            
            // Auth
            let auth_msg = serde_json::json!({
                "event": "auth",
                "token": token
            });
            if let Err(e) = sink.send(Message::Text(auth_msg.to_string().into())).await {
                println!("❌ Auth send error: {}", e);
                return;
            }
            println!("🔑 Auth sent");
            
            // Subscribe to orderbook
            let sub_msg = serde_json::json!({
                "event": "subscribe",
                "figi": figi,
                "depth": 10,
                "type": "orderbook"
            });
            if let Err(e) = sink.send(Message::Text(sub_msg.to_string().into())).await {
                println!("❌ Subscribe error: {}", e);
                return;
            }
            println!("📚 Subscribed to orderbook for {}", figi);
            
            // Listen for messages
            while let Some(msg) = stream.next().await {
                match msg {
                    Ok(Message::Text(text)) => {
                        println!("📩 {}", text);
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
            println!("Hint: This might be a TLS/certificate issue. Try the insecure version below.");
        }
    }
}