use super::price::Price;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Level {
    pub price: Price,
    pub volume: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderBook {
    pub figi: String,
    pub bids: Vec<Level>,
    pub asks: Vec<Level>,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

impl OrderBook {
    pub fn new(figi: String) -> Self {
        Self {
            figi,
            bids: Vec::with_capacity(10),
            asks: Vec::with_capacity(10),
            timestamp: chrono::Utc::now(),
        }
    }

    pub fn update_bid(&mut self, price: Price, volume: u64) {
        let pos = self
            .bids
            .iter()
            .position(|l| l.price < price)
            .unwrap_or(self.bids.len());
        if pos < self.bids.len() && self.bids[pos].price == price {
            if volume == 0 {
                self.bids.remove(pos);
            } else {
                self.bids[pos].volume = volume;
            }
        } else if volume > 0 {
            self.bids.insert(pos, Level { price, volume });
        }
        self.bids.truncate(10);
    }

    pub fn update_ask(&mut self, price: Price, volume: u64) {
        let pos = self
            .asks
            .iter()
            .position(|l| l.price > price)
            .unwrap_or(self.asks.len());
        if pos < self.asks.len() && self.asks[pos].price == price {
            if volume == 0 {
                self.asks.remove(pos);
            } else {
                self.asks[pos].volume = volume;
            }
        } else if volume > 0 {
            self.asks.insert(pos, Level { price, volume });
        }
        self.asks.truncate(10);
    }

    pub fn pressure(&self) -> f64 {
        let total_bid: u64 = self.bids.iter().map(|l| l.volume).sum();
        let total_ask: u64 = self.asks.iter().map(|l| l.volume).sum();
        if total_bid == 0 || total_ask == 0 {
            return 0.0;
        }
        let weighted_ask: f64 = self
            .asks
            .iter()
            .enumerate()
            .map(|(i, l)| (l.volume as f64 / total_ask as f64) * (i + 1) as f64)
            .sum();
        let weighted_bid: f64 = self
            .bids
            .iter()
            .enumerate()
            .map(|(i, l)| (l.volume as f64 / total_bid as f64) * (10 - i) as f64)
            .sum();
        weighted_ask - weighted_bid
    }

    pub fn signal(&self) -> i8 {
        let p = self.pressure();
        if p > 0.3 {
            1
        } else if p < -0.3 {
            -1
        } else {
            0
        }
    }
}
