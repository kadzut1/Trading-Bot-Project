use reqwest::Client;
use serde_json::json;
use trading_bot_rust::domain::{Price, OrderBook};

#[tokio::main]
async fn main() {
    let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();
    let token = std::env::var("TINVEST_TOKEN").expect("TINVEST_TOKEN not set");
    let figi = "BBG004730N88";
    
    let client = Client::builder()
        .danger_accept_invalid_certs(true)
        .build()
        .unwrap();
    
    let url = "https://sandbox-invest-public-api.tbank.ru/rest/tinkoff.public.invest.api.contract.v1.MarketDataService/GetOrderBook";
    
    let body = json!({
        "figi": figi,
        "depth": 10
    });
    
    println!("📚 Запрашиваем стакан для {}", figi);
    
    match client.post(url)
        .header("Authorization", format!("Bearer {}", token))
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await
    {
        Ok(resp) => {
            if resp.status().is_success() {
                let data: serde_json::Value = resp.json().await.unwrap();
                let mut orderbook = OrderBook::new(figi.to_string());
                
                if let Some(bids) = data["bids"].as_array() {
                    for bid in bids {
                        // T-Invest возвращает цену в объекте { units, nano }
                        let price_units = bid["price"]["units"].as_str().unwrap_or("0").parse::<i64>().unwrap_or(0);
                        let price_nano = bid["price"]["nano"].as_i64().unwrap_or(0);
                        let price_f = price_units as f64 + price_nano as f64 / 1_000_000_000.0;
                        let price = Price::from_float(price_f);
                        
                        let volume = bid["quantity"].as_str().unwrap_or("0").parse::<u64>().unwrap_or(0);
                        orderbook.update_bid(price, volume);
                    }
                }
                
                if let Some(asks) = data["asks"].as_array() {
                    for ask in asks {
                        let price_units = ask["price"]["units"].as_str().unwrap_or("0").parse::<i64>().unwrap_or(0);
                        let price_nano = ask["price"]["nano"].as_i64().unwrap_or(0);
                        let price_f = price_units as f64 + price_nano as f64 / 1_000_000_000.0;
                        let price = Price::from_float(price_f);
                        
                        let volume = ask["quantity"].as_str().unwrap_or("0").parse::<u64>().unwrap_or(0);
                        orderbook.update_ask(price, volume);
                    }
                }
                
                let pressure = orderbook.pressure();
                let signal = orderbook.signal();
                
                println!("=== OrderBook Analysis ===");
                println!("Bids:");
                for (i, bid) in orderbook.bids.iter().enumerate() {
                    println!("  {}: {:.2} x {}", i+1, bid.price.as_float(), bid.volume);
                }
                println!("Asks:");
                for (i, ask) in orderbook.asks.iter().enumerate() {
                    println!("  {}: {:.2} x {}", i+1, ask.price.as_float(), ask.volume);
                }
                println!("Pressure: {:.4}", pressure);
                println!("Signal: {}", match signal { 1 => "🔥 BULLISH (BUY)", -1 => "❄️ BEARISH (SELL)", _ => "⚖️ NEUTRAL" });
            } else {
                println!("Error: {}", resp.status());
            }
        }
        Err(e) => println!("Error: {}", e),
    }
}