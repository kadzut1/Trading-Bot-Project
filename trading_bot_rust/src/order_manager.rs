use log::info;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;

pub struct RateLimiter {
    tokens: u32,
    max_tokens: u32,
    refill_rate: u32,
    last_refill: Instant,
}

impl RateLimiter {
    pub fn new(max_tokens: u32, refill_rate: u32) -> Self {
        Self {
            tokens: max_tokens,
            max_tokens,
            refill_rate,
            last_refill: Instant::now(),
        }
    }

    pub async fn acquire(&mut self) {
        self.refill();
        while self.tokens == 0 {
            tokio::time::sleep(Duration::from_millis(100)).await;
            self.refill();
        }
        self.tokens -= 1;
    }

    fn refill(&mut self) {
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_refill).as_secs_f32();
        let new_tokens = (elapsed * self.refill_rate as f32) as u32;
        if new_tokens > 0 {
            self.tokens = (self.tokens + new_tokens).min(self.max_tokens);
            self.last_refill = now;
        }
    }
}

pub struct OrderManager {
    rate_limiter: Arc<Mutex<RateLimiter>>,
    pending_orders: Arc<Mutex<Vec<String>>>,
}

impl OrderManager {
    pub fn new() -> Self {
        Self {
            rate_limiter: Arc::new(Mutex::new(RateLimiter::new(5, 5))),
            pending_orders: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub async fn post_order(
        &self,
        figi: &str,
        quantity: i64,
        price: f64,
        side: &str,
    ) -> Result<String, anyhow::Error> {
        let mut limiter = self.rate_limiter.lock().await;
        limiter.acquire().await;

        info!(
            "✅ Order posted: {} {} @ {} for {}",
            side, quantity, price, figi
        );

        let order_id = format!("order_{}_{}", chrono::Utc::now().timestamp(), figi);
        let mut orders = self.pending_orders.lock().await;
        orders.push(order_id.clone());

        Ok(order_id)
    }
}
