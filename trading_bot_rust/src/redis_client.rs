use redis::AsyncCommands;
use serde_json::json;

pub struct RedisClient {
    conn: redis::aio::Connection,
}

impl RedisClient {
    pub async fn connect() -> Result<Self, redis::RedisError> {
        let client = redis::Client::open("redis://localhost:6379")?;
        let conn = client.get_async_connection().await?;
        Ok(Self { conn })
    }

    // Публикация сигнала
    pub async fn publish_signal(&mut self, decision: &str, price: f64, pressure: f64) -> Result<(), redis::RedisError> {
        let msg = json!({
            "timestamp": chrono::Utc::now().to_rfc3339(),
            "decision": decision,
            "price": price,
            "pressure": pressure,
            "sentiment": 0.0,  // TODO: из Python
            "bayesian": 0.0,   // TODO: из Python
        });
        let _: () = self.conn.publish("trading.signals", msg.to_string()).await?;
        println!("📡 Signal published: {}", decision);
        Ok(())
    }

    // Публикация свечи
    pub async fn publish_candle(&mut self, figi: &str, close: f64, volume: u64) -> Result<(), redis::RedisError> {
        let msg = json!({
            "figi": figi,
            "timestamp": chrono::Utc::now().to_rfc3339(),
            "close": close,
            "volume": volume,
        });
        let _: () = self.conn.publish("market.candles", msg.to_string()).await?;
        Ok(())
    }
}