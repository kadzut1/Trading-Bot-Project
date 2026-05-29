use super::price::Price;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Candle {
    pub figi: String,
    pub time: DateTime<Utc>,
    pub open: Price,
    pub high: Price,
    pub low: Price,
    pub close: Price,
    pub volume: u64,
    pub volume_buy: u64,
    pub volume_sell: u64,
    pub is_complete: bool,
}

impl Candle {
    pub fn from_tinvest_json(figi: String, data: &serde_json::Value) -> anyhow::Result<Self> {
        let time = data["time"]
            .as_str()
            .unwrap_or("")
            .parse::<DateTime<Utc>>()?;
        let open_units = data["open"]["units"]
            .as_str()
            .unwrap_or("0")
            .parse::<i64>()?;
        let open_nano = data["open"]["nano"].as_i64().unwrap_or(0);
        let open = Price::from_rubles(open_units as f64 + open_nano as f64 / 1_000_000_000.0);
        let high_units = data["high"]["units"]
            .as_str()
            .unwrap_or("0")
            .parse::<i64>()?;
        let high_nano = data["high"]["nano"].as_i64().unwrap_or(0);
        let high = Price::from_rubles(high_units as f64 + high_nano as f64 / 1_000_000_000.0);
        let low_units = data["low"]["units"]
            .as_str()
            .unwrap_or("0")
            .parse::<i64>()?;
        let low_nano = data["low"]["nano"].as_i64().unwrap_or(0);
        let low = Price::from_rubles(low_units as f64 + low_nano as f64 / 1_000_000_000.0);
        let close_units = data["close"]["units"]
            .as_str()
            .unwrap_or("0")
            .parse::<i64>()?;
        let close_nano = data["close"]["nano"].as_i64().unwrap_or(0);
        let close = Price::from_rubles(close_units as f64 + close_nano as f64 / 1_000_000_000.0);
        let volume = data["volume"].as_str().unwrap_or("0").parse::<u64>()?;
        let volume_buy = data["volumeBuy"]
            .as_str()
            .unwrap_or("0")
            .parse::<u64>()
            .unwrap_or(0);
        let volume_sell = data["volumeSell"]
            .as_str()
            .unwrap_or("0")
            .parse::<u64>()
            .unwrap_or(0);
        let is_complete = data["isComplete"].as_bool().unwrap_or(false);

        Ok(Candle {
            figi,
            time,
            open,
            high,
            low,
            close,
            volume,
            volume_buy,
            volume_sell,
            is_complete,
        })
    }
}
