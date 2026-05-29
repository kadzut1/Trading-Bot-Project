use reqwest::Client;
use serde_json::json;
use chrono::{Utc, Duration};
use trading_bot_rust::domain::Candle;

#[tokio::main]
async fn main() {
    let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();
    let token = std::env::var("TINVEST_TOKEN").expect("TINVEST_TOKEN not set");
    let figi = "BBG004730N88";
    let client = Client::builder().danger_accept_invalid_certs(true).build().unwrap();
    let url = "https://sandbox-invest-public-api.tbank.ru/rest/tinkoff.public.invest.api.contract.v1.MarketDataService/GetCandles";
    let now = Utc::now();
    let from = now - Duration::days(30);
    let body = json!({ "figi": figi, "from": from.to_rfc3339(), "to": now.to_rfc3339(), "interval": 5 });
    println!("Fetching candles...");
    match client.post(url).header("Authorization", format!("Bearer {}", token)).header("Content-Type", "application/json").json(&body).send().await {
        Ok(resp) => {
            if resp.status().is_success() {
                let data: serde_json::Value = resp.json().await.unwrap();
                if let Some(candles) = data["candles"].as_array() {
                    println!("Got {} candles", candles.len());
                    for (i, candle_data) in candles.iter().enumerate() {
                        if let Ok(candle) = Candle::from_tinvest_json(figi.to_string(), candle_data) {
                            if candle.is_complete {
                                println!("{}: {} Close: {:.2} Vol: {}", i+1, candle.time.format("%Y-%m-%d"), candle.close.as_float(), candle.volume);
                            }
                        }
                    }
                }
            } else { println!("Error: {}", resp.status()); }
        }
        Err(e) => { println!("Error: {}", e); }
    }
}