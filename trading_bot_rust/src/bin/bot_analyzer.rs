use reqwest::Client;
use serde_json::json;
use chrono::{Utc, Duration};
use trading_bot_rust::domain::{Candle, Analyzer};

#[tokio::main]
async fn main() {
    let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();
    let token = std::env::var("TINVEST_TOKEN").expect("TINVEST_TOKEN not set");
    let figi = "BBG004730N88";
    
    let client = Client::builder().danger_accept_invalid_certs(true).build().unwrap();
    let url = "https://sandbox-invest-public-api.tbank.ru/rest/tinkoff.public.invest.api.contract.v1.MarketDataService/GetCandles";
    
    let now = Utc::now();
    let from = now - Duration::days(60);
    let body = json!({ "figi": figi, "from": from.to_rfc3339(), "to": now.to_rfc3339(), "interval": 5 });
    
    println!("Fetching candles for analysis...");
    
    match client.post(url).header("Authorization", format!("Bearer {}", token)).header("Content-Type", "application/json").json(&body).send().await {
        Ok(resp) => {
            if resp.status().is_success() {
                let data: serde_json::Value = resp.json().await.unwrap();
                if let Some(candles_data) = data["candles"].as_array() {
                    let mut candles = Vec::new();
                    for candle_data in candles_data {
                        if let Ok(candle) = Candle::from_tinvest_json(figi.to_string(), candle_data) {
                            if candle.is_complete {
                                candles.push(candle);
                            }
                        }
                    }
                    
                    println!("\n=== ANALYSIS ===\n");
                    
                    let analyzer = Analyzer::new(14);
                    let atr = analyzer.calculate_atr(&candles);
                    println!("ATR (14): {:.4}", atr);
                    
                    let trend = Analyzer::trend(&candles, 14);
                    let trend_str = match trend { 1 => "UP", -1 => "DOWN", _ => "SIDEWAYS" };
                    println!("Trend: {}", trend_str);
                    
                    println!("\nLast 10 candles signal:");
                    println!("{:-<60}", "");
                    println!("{:>3} | {:^12} | {:>8} | {:>8} | {:>5}", "#", "Date", "Close", "Signal", "Trend");
                    println!("{:-<60}", "");
                    
                    let start = if candles.len() > 10 { candles.len() - 10 } else { 0 };
                    for i in start..candles.len() {
                        let signal = Analyzer::candle_signal(&candles[i]);
                        let signal_str = match signal { 2 => "🔥B", 1 => "↑B", -1 => "↓S", -2 => "🔥S", _ => "●" };
                        let candle_trend = Analyzer::trend(&candles[0..=i], 14);
                        let trend_str = match candle_trend { 1 => "UP", -1 => "DOWN", _ => "→" };
                        println!("{:>3} | {} | {:>8.2} | {:>5} | {:>5}", 
                            i+1, 
                            candles[i].time.format("%Y-%m-%d"), 
                            candles[i].close.as_float(),
                            signal_str,
                            trend_str
                        );
                    }
                    println!("{:-<60}", "");
                }
            } else { println!("Error: {}", resp.status()); }
        }
        Err(e) => { println!("Error: {}", e); }
    }
}