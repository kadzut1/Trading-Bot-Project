use chrono::{Duration, Utc};
use reqwest::Client;
use serde_json::json;
use trading_bot_rust::domain::{Analyzer, Candle};

#[tokio::main]
async fn main() {
    let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();
    let token = std::env::var("TINVEST_TOKEN").expect("TINVEST_TOKEN not set");
    let figi = "BBG004730N88";

    let client = Client::builder()
        .danger_accept_invalid_certs(true)
        .build()
        .unwrap();

    let url = "https://sandbox-invest-public-api.tbank.ru/rest/tinkoff.public.invest.api.contract.v1.MarketDataService/GetCandles";

    let now = Utc::now();
    let from = now - Duration::days(30);

    let body = json!({
        "figi": figi,
        "from": from.to_rfc3339(),
        "to": now.to_rfc3339(),
        "interval": 5
    });

    let resp = client
        .post(url)
        .header("Authorization", format!("Bearer {}", token))
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await
        .unwrap();

    if resp.status().is_success() {
        let data: serde_json::Value = resp.json().await.unwrap();
        if let Some(candles_data) = data["candles"].as_array() {
            let mut candles = Vec::new();
            for cd in candles_data {
                if let Ok(c) = Candle::from_tinvest_json(figi.to_string(), cd) {
                    if c.is_complete {
                        candles.push(c);
                    }
                }
            }

            let analyzer = Analyzer::new(14);
            let atr = analyzer.calculate_atr(&candles);
            let trend = Analyzer::trend(&candles, 14);
            let last = candles.last().unwrap();
            let signal = Analyzer::candle_signal(last);

            println!("=== T-Invest Bot ===");
            println!("Price: {:.2}", last.close.as_float());
            println!("ATR: {:.2}", atr);
            println!(
                "Trend: {}",
                if trend > 0 {
                    "UP"
                } else if trend < 0 {
                    "DOWN"
                } else {
                    "SIDEWAYS"
                }
            );
            println!(
                "Signal: {}",
                match signal {
                    2 => "STRONG BUY",
                    1 => "BUY",
                    -1 => "SELL",
                    -2 => "STRONG SELL",
                    _ => "HOLD",
                }
            );
        }
    } else {
        println!("Error: {}", resp.status());
    }
}
