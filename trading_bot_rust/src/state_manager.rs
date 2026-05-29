use redis::{AsyncCommands, Client};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct TradingState {
    pub capital: f64,
    pub daily_pnl: f64,
    pub last_reset_day: String,
    pub active_position: Option<StoredPosition>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct StoredPosition {
    pub side: String,
    pub entry_price: f64,
    pub quantity: i64,
    pub stop_loss: f64,
    pub take_profit: f64,
    pub entry_time: String,
    pub figi: String,
}

#[derive(Clone)]
pub struct StateManager {
    redis_client: Client,
    figi: String,
}

impl StateManager {
    pub fn new(redis_url: &str, figi: &str) -> Self {
        let redis_client = Client::open(redis_url).unwrap();
        Self {
            redis_client,
            figi: figi.to_string(),
        }
    }

    pub async fn load_state(&self) -> Option<TradingState> {
        let mut conn = self.redis_client.get_async_connection().await.ok()?;
        let key = format!("trading_state:{}", self.figi);
        let data: Option<String> = conn.get(&key).await.ok()?;
        data.and_then(|d| serde_json::from_str(&d).ok())
    }

    pub async fn save_state(&self, state: &TradingState) -> Result<(), redis::RedisError> {
        let mut conn = self.redis_client.get_async_connection().await?;
        let key = format!("trading_state:{}", self.figi);
        let data = serde_json::to_string(state).unwrap();
        let _: () = conn.set_ex(&key, data, 86400).await?; // храним 24 часа
        Ok(())
    }

    pub async fn clear_state(&self) -> Result<(), redis::RedisError> {
        let mut conn = self.redis_client.get_async_connection().await?;
        let key = format!("trading_state:{}", self.figi);
        let _: () = conn.del(&key).await?;
        Ok(())
    }
}
