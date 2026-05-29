use futures_util::{SinkExt, StreamExt};
use native_tls::TlsConnector;
use serde_json::json;
use std::sync::Arc;
use tokio::net::TcpStream;
use tokio::sync::Mutex;
use tokio_tungstenite::Connector;
use tokio_tungstenite::{connect_async, tungstenite::Message, MaybeTlsStream};

pub type WsStream = tokio_tungstenite::WebSocketStream<MaybeTlsStream<TcpStream>>;

pub struct WebSocketClient {
    sink: Arc<Mutex<futures_util::stream::SplitSink<WsStream, Message>>>,
    stream: Arc<Mutex<futures_util::stream::SplitStream<WsStream>>>,
    token: String,
}

impl WebSocketClient {
    pub async fn connect(token: &str) -> Result<Self, anyhow::Error> {
        let url = "wss://sandbox-invest-public-api.tbank.ru/ws";
        println!("🔌 Connecting to WebSocket: {}", url);

        // Создаём TLS коннектор с отключенной проверкой сертификата
        let tls_connector = TlsConnector::builder()
            .danger_accept_invalid_certs(true)
            .build()?;

        // Используем connect_async_with_config
        let request = url.parse::<http::Uri>()?;
        let mut config = tokio_tungstenite::tungstenite::protocol::WebSocketConfig::default();

        let (ws_stream, _) =
            tokio_tungstenite::connect_async_with_config(request, Some(config), false).await?;
        let (sink, stream) = ws_stream.split();

        let client = Self {
            sink: Arc::new(Mutex::new(sink)),
            stream: Arc::new(Mutex::new(stream)),
            token: token.to_string(),
        };

        client.auth().await?;

        Ok(client)
    }

    async fn auth(&self) -> Result<(), anyhow::Error> {
        let auth_msg = json!({
            "event": "auth",
            "token": self.token
        });
        let mut sink = self.sink.lock().await;
        sink.send(Message::Text(auth_msg.to_string().into()))
            .await?;
        println!("🔑 Auth sent");
        Ok(())
    }

    pub async fn subscribe_candles(&self, figi: &str, interval: &str) -> Result<(), anyhow::Error> {
        let msg = json!({
            "event": "subscribe",
            "figi": figi,
            "interval": interval
        });
        let mut sink = self.sink.lock().await;
        sink.send(Message::Text(msg.to_string().into())).await?;
        println!("📊 Subscribed to candles: {}", figi);
        Ok(())
    }

    pub async fn subscribe_orderbook(&self, figi: &str, depth: u32) -> Result<(), anyhow::Error> {
        let msg = json!({
            "event": "subscribe",
            "figi": figi,
            "depth": depth,
            "type": "orderbook"
        });
        let mut sink = self.sink.lock().await;
        sink.send(Message::Text(msg.to_string().into())).await?;
        println!("📚 Subscribed to orderbook: {} (depth {})", figi, depth);
        Ok(())
    }

    pub async fn next_message(&self) -> Option<String> {
        let mut stream = self.stream.lock().await;
        match stream.next().await {
            Some(Ok(Message::Text(text))) => Some(text.to_string()),
            Some(Err(e)) => {
                eprintln!("WebSocket error: {}", e);
                None
            }
            _ => None,
        }
    }

    pub async fn run<F>(&self, mut callback: F) -> Result<(), anyhow::Error>
    where
        F: FnMut(String) + Send + 'static,
    {
        loop {
            if let Some(msg) = self.next_message().await {
                callback(msg);
            }
            tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
        }
    }
}
