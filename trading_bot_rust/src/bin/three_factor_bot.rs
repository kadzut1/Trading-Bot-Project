use chrono::{Duration, Utc};
use reqwest::Client;
use serde_json::json;
use tokio::time;
use trading_bot_rust::domain::{Analyzer, Candle, OrderBook, Price};
use trading_bot_rust::redis_client::RedisClient;
// ============ PROMETHEUS METRICS ============
use lazy_static::lazy_static;
use prometheus::{
    register_counter, register_gauge, register_histogram, Counter, Encoder, Gauge, Histogram,
    TextEncoder,
};
use trading_bot_rust::state_manager::{StateManager, StoredPosition, TradingState};
use warp::Filter;

lazy_static! {
    static ref TRADES_TOTAL: Counter =
        register_counter!("trades_total", "Total number of trades").unwrap();
    static ref LONG_TRADES: Counter =
        register_counter!("long_trades_total", "Total number of long trades").unwrap();
    static ref SHORT_TRADES: Counter =
        register_counter!("short_trades_total", "Total number of short trades").unwrap();
    static ref PORTFOLIO_VALUE: Gauge =
        register_gauge!("portfolio_value", "Current portfolio value in RUB").unwrap();
    static ref CURRENT_POSITION_SIZE: Gauge =
        register_gauge!("current_position_size", "Current number of shares held").unwrap();
    static ref DAILY_PNL: Gauge = register_gauge!("daily_pnl", "Daily profit/loss in RUB").unwrap();
    static ref SIGNAL_STRENGTH: Histogram = register_histogram!(
        "signal_strength",
        "Strength of trading signals",
        vec![0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8, 0.9, 1.0]
    )
    .unwrap();
    static ref PRESSURE_VALUE: Gauge =
        register_gauge!("orderbook_pressure", "Current orderbook pressure").unwrap();
    static ref SENTIMENT_VALUE: Gauge =
        register_gauge!("news_sentiment", "Current news sentiment").unwrap();
    static ref API_LATENCY: Histogram = register_histogram!(
        "api_latency_seconds",
        "API request latency in seconds",
        vec![0.01, 0.05, 0.1, 0.5, 1.0, 2.0, 5.0]
    )
    .unwrap();
}

async fn metrics_server() {
    let metrics_route = warp::path("metrics").map(|| {
        let encoder = TextEncoder::new();
        let metric_families = prometheus::gather();
        let mut buffer = vec![];
        encoder.encode(&metric_families, &mut buffer).unwrap();
        String::from_utf8(buffer).unwrap()
    });
    warp::serve(metrics_route).run(([0, 0, 0, 0], 8080)).await;
}

// ============ РИСК-МЕНЕДЖМЕНТ ============
struct RiskManager {
    capital: f64,
    daily_pnl: f64,
    last_reset_day: chrono::DateTime<Utc>,
    active_position: Option<Position>,
    state_manager: Option<StateManager>,
    figi: String,
}

struct Position {
    side: String,
    entry_price: f64,
    quantity: i64,
    stop_loss: f64,
    take_profit: f64,
}

impl RiskManager {
    fn new(initial_capital: f64, figi: &str, state_manager: Option<StateManager>) -> Self {
        Self {
            capital: initial_capital,
            daily_pnl: 0.0,
            last_reset_day: Utc::now(),
            active_position: None,
            state_manager,
            figi: figi.to_string(),
        }
    }

    async fn save_state(&self) {
        if let Some(ref sm) = self.state_manager {
            let state = TradingState {
                capital: self.capital,
                daily_pnl: self.daily_pnl,
                last_reset_day: self.last_reset_day.to_rfc3339(),
                active_position: self.active_position.as_ref().map(|p| StoredPosition {
                    side: p.side.clone(),
                    entry_price: p.entry_price,
                    quantity: p.quantity,
                    stop_loss: p.stop_loss,
                    take_profit: p.take_profit,
                    entry_time: Utc::now().to_rfc3339(),
                    figi: self.figi.clone(),
                }),
            };
            let _ = sm.save_state(&state).await;
        }
    }

    fn reset_daily_pnl(&mut self) {
        let today = Utc::now().date_naive();
        if self.last_reset_day.date_naive() != today {
            self.daily_pnl = 0.0;
            self.last_reset_day = Utc::now();
            println!("📅 Daily PnL reset");
        }
    }

    fn check_daily_limit(&self) -> bool {
        if self.daily_pnl < -self.capital * 0.03 {
            println!("🔴 DAILY LOSS LIMIT HIT! Stopping trades for today");
            return false;
        }
        true
    }

    fn calculate_position_size(&self, confidence: f64, atr: f64, price: f64) -> i64 {
        let kelly_fraction = 0.25;
        let base_kelly = self.capital * (confidence - 0.5) * 2.0 * kelly_fraction;

        let aggressiveness = if confidence > 0.8 {
            3.0
        } else if confidence > 0.7 {
            2.0
        } else if confidence > 0.6 {
            1.5
        } else {
            1.0
        };

        let volatility_adj = if atr > 0.0 { (1.0 / atr) * 10.0 } else { 1.0 };
        let mut position_value = base_kelly * aggressiveness * volatility_adj;
        position_value = position_value.clamp(self.capital * 0.01, self.capital * 0.10);
        let quantity = (position_value / price).max(1.0) as i64;
        quantity.clamp(1, 500)
    }

    fn calculate_stop_loss(entry_price: f64, atr: f64, side: &str) -> f64 {
        if side == "LONG" {
            entry_price - atr * 1.5
        } else {
            entry_price + atr * 1.5
        }
    }

    fn calculate_take_profit(entry_price: f64, atr: f64, side: &str) -> f64 {
        if side == "LONG" {
            entry_price + atr * 3.0
        } else {
            entry_price - atr * 3.0
        }
    }

    async fn open_position(
        &mut self,
        side: String,
        price: f64,
        quantity: i64,
        atr: f64,
        confidence: f64,
    ) -> bool {
        if self.active_position.is_some() {
            println!("⚠️ Position already open");
            return false;
        }

        if !self.check_daily_limit() {
            return false;
        }

        let stop_loss = Self::calculate_stop_loss(price, atr, &side);
        let take_profit = Self::calculate_take_profit(price, atr, &side);
        let position_size_pct = (quantity as f64 * price) / self.capital * 100.0;

        let confidence_level = if confidence > 0.8 {
            "🔥 VERY HIGH"
        } else if confidence > 0.7 {
            "⭐ HIGH"
        } else if confidence > 0.6 {
            "✓ MEDIUM"
        } else {
            "◌ LOW"
        };

        println!("\n📊 POSITION OPENED");
        println!("   Side: {}", side);
        println!("   Entry: {:.2}", price);
        println!("   Quantity: {}", quantity);
        println!("   Position size: {:.2}% of capital", position_size_pct);
        println!(
            "   Confidence: {} ({:.0}%)",
            confidence_level,
            confidence * 100.0
        );
        println!("   Stop Loss: {:.2} (1.5x ATR)", stop_loss);
        println!("   Take Profit: {:.2} (3x ATR)", take_profit);
        println!(
            "   Risk: {:.2} RUB",
            (price - stop_loss).abs() * quantity as f64
        );
        println!(
            "   Reward: {:.2} RUB",
            (take_profit - price).abs() * quantity as f64
        );
        println!("   Risk/Reward: 1:2");

        self.active_position = Some(Position {
            side: side.clone(),
            entry_price: price,
            quantity,
            stop_loss,
            take_profit,
        });

        CURRENT_POSITION_SIZE.set(quantity as f64);
        TRADES_TOTAL.inc();
        if side == "LONG" {
            LONG_TRADES.inc();
        } else {
            SHORT_TRADES.inc();
        }
        PORTFOLIO_VALUE.set(self.capital - (quantity as f64 * price));

        self.save_state().await;

        true
    }

    async fn close_position(&mut self, current_price: f64, reason: &str) -> Option<f64> {
        if let Some(pos) = self.active_position.take() {
            let pnl = if pos.side == "LONG" {
                (current_price - pos.entry_price) * pos.quantity as f64
            } else {
                (pos.entry_price - current_price) * pos.quantity as f64
            };

            self.daily_pnl += pnl;
            self.capital += pnl;

            TRADES_TOTAL.inc();

            PORTFOLIO_VALUE.set(self.capital);
            DAILY_PNL.set(self.daily_pnl);
            CURRENT_POSITION_SIZE.set(0.0);

            println!("\n📊 POSITION CLOSED");
            println!("   Reason: {}", reason);
            println!("   PnL: {:.2} RUB", pnl);
            println!("   Daily PnL: {:.2} RUB", self.daily_pnl);
            println!("   Capital: {:.2} RUB", self.capital);

            self.save_state().await;

            Some(pnl)
        } else {
            None
        }
    }

    async fn check_stop_loss_take_profit(&mut self, current_price: f64) -> bool {
        if let Some(pos) = &self.active_position {
            if pos.side == "LONG" {
                if current_price <= pos.stop_loss {
                    self.close_position(current_price, "STOP LOSS HIT").await;
                    return true;
                } else if current_price >= pos.take_profit {
                    self.close_position(current_price, "TAKE PROFIT HIT").await;
                    return true;
                }
            } else {
                if current_price >= pos.stop_loss {
                    self.close_position(current_price, "STOP LOSS HIT").await;
                    return true;
                } else if current_price <= pos.take_profit {
                    self.close_position(current_price, "TAKE PROFIT HIT").await;
                    return true;
                }
            }
        }
        false
    }

    fn get_active_position(&self) -> Option<&Position> {
        self.active_position.as_ref()
    }
}

// ============ НОВОСТНОЙ СЕНТИМЕНТ ============
async fn get_aggregated_sentiment(client: &Client) -> f64 {
    let url = "http://localhost:8001/sentiment";
    match client
        .get(url)
        .timeout(std::time::Duration::from_secs(2))
        .send()
        .await
    {
        Ok(resp) if resp.status().is_success() => {
            let json: serde_json::Value = resp.json().await.unwrap_or(serde_json::json!({}));
            let sentiment = json["sentiment"].as_str().unwrap_or("neutral");
            let score = json["score"].as_f64().unwrap_or(0.0);
            match sentiment {
                "positive" => score,
                "negative" => -score,
                _ => 0.0,
            }
        }
        _ => 0.0,
    }
}

// ============ БАЙЕСОВСКАЯ РЕГРЕССИЯ ============
async fn get_bayesian(
    price: f64,
    atr: f64,
    trend: i8,
    pressure: f64,
    sentiment: f64,
    client: &Client,
) -> f64 {
    let url = format!(
        "http://localhost:8002/probability?price={}&atr={}&trend={}&pressure={}&sentiment={}",
        price, atr, trend, pressure, sentiment
    );
    match client
        .get(&url)
        .timeout(std::time::Duration::from_secs(2))
        .send()
        .await
    {
        Ok(resp) if resp.status().is_success() => {
            let json: serde_json::Value = resp.json().await.unwrap_or(serde_json::json!({}));
            json["signal"].as_f64().unwrap_or(0.0)
        }
        _ => 0.0,
    }
}

// ============ LIGHTGBM ============
async fn get_lgbm(
    price: f64,
    atr: f64,
    trend: i8,
    pressure: f64,
    sentiment: f64,
    client: &Client,
) -> f64 {
    let url = format!(
        "http://localhost:8003/predict?price={}&atr={}&trend={}&pressure={}&sentiment={}",
        price, atr, trend, pressure, sentiment
    );
    match client
        .get(&url)
        .timeout(std::time::Duration::from_secs(2))
        .send()
        .await
    {
        Ok(resp) if resp.status().is_success() => {
            let json: serde_json::Value = resp.json().await.unwrap_or(serde_json::json!({}));
            json["signal"].as_f64().unwrap_or(0.0)
        }
        _ => 0.0,
    }
}

// ============ СОХРАНЕНИЕ В CLICKHOUSE ============
async fn save_candle_to_clickhouse(figi: &str, candle: &Candle, client: &Client) {
    let query = format!(
        "INSERT INTO candles (figi, timestamp, open, high, low, close, volume) VALUES ('{}','{}',{},{},{},{},{})",
        figi,
        candle.time.format("%Y-%m-%d %H:%M:%S"),
        candle.open.as_float(),
        candle.high.as_float(),
        candle.low.as_float(),
        candle.close.as_float(),
        candle.volume
    );
    let url = format!(
        "http://localhost:8123/?query={}",
        urlencoding::encode(&query)
    );
    let _ = client.get(&url).send().await;
}

async fn save_orderbook_to_clickhouse(figi: &str, orderbook: &OrderBook, client: &Client) {
    let query = format!(
        "INSERT INTO orderbook (figi, timestamp, pressure, bid_volume_ratio, ask_volume_ratio) VALUES ('{}','{}',{},{},{})",
        figi,
        orderbook.timestamp.format("%Y-%m-%d %H:%M:%S"),
        orderbook.pressure(),
        if orderbook.bids.len() > 0 { orderbook.bids.iter().map(|b| b.volume).sum::<u64>() as f64 / 1000000.0 } else { 0.0 },
        if orderbook.asks.len() > 0 { orderbook.asks.iter().map(|a| a.volume).sum::<u64>() as f64 / 1000000.0 } else { 0.0 }
    );
    let url = format!(
        "http://localhost:8123/?query={}",
        urlencoding::encode(&query)
    );
    let _ = client.get(&url).send().await;
}

// ============ СОЗДАНИЕ И ПОПОЛНЕНИЕ СЧЁТА ============
async fn create_and_fund_account(token: &str) -> String {
    let client = reqwest::Client::builder()
        .danger_accept_invalid_certs(true)
        .build()
        .unwrap();

    let accounts_url = "https://sandbox-invest-public-api.tbank.ru/rest/tinkoff.public.invest.api.contract.v1.UsersService/GetAccounts";

    match client
        .post(accounts_url)
        .header("Authorization", format!("Bearer {}", token))
        .header("Content-Type", "application/json")
        .json(&json!({}))
        .send()
        .await
    {
        Ok(resp) => {
            let text = resp.text().await.unwrap_or_default();
            println!("📊 GetAccounts response: {}", &text[..text.len().min(200)]);

            if let Ok(data) = serde_json::from_str::<serde_json::Value>(&text) {
                if let Some(accounts) = data["accounts"].as_array() {
                    if let Some(first) = accounts.first() {
                        if let Some(id) = first["id"].as_str() {
                            println!("✅ Found existing account: {}", id);
                            return id.to_string();
                        }
                    }
                }
            }
        }
        Err(e) => println!("⚠️ GetAccounts error: {}", e),
    }

    println!("📝 No accounts found, creating new...");
    let create_url = "https://sandbox-invest-public-api.tbank.ru/rest/tinkoff.public.invest.api.contract.v1.SandboxService/OpenSandboxAccount";

    match client
        .post(create_url)
        .header("Authorization", format!("Bearer {}", token))
        .header("Content-Type", "application/json")
        .json(&json!({}))
        .send()
        .await
    {
        Ok(resp) => {
            let text = resp.text().await.unwrap_or_default();
            println!("📊 CreateAccount response: {}", text);

            if let Ok(data) = serde_json::from_str::<serde_json::Value>(&text) {
                if let Some(id) = data["accountId"].as_str() {
                    println!("✅ Created account: {}", id);

                    let fund_url = "https://sandbox-invest-public-api.tbank.ru/rest/tinkoff.public.invest.api.contract.v1.SandboxService/SandboxPayIn";
                    let fund_body = json!({
                        "accountId": id,
                        "amount": {
                            "units": 100000,
                            "nano": 0
                        }
                    });

                    match client
                        .post(fund_url)
                        .header("Authorization", format!("Bearer {}", token))
                        .header("Content-Type", "application/json")
                        .json(&fund_body)
                        .send()
                        .await
                    {
                        Ok(fund_resp) => {
                            if fund_resp.status().is_success() {
                                println!("✅ New account funded with 100,000 RUB");
                            }
                        }
                        Err(e) => println!("⚠️ Fund error: {}", e),
                    }

                    return id.to_string();
                }
            }
        }
        Err(e) => println!("❌ Create account error: {}", e),
    }

    println!("⚠️ Could not create/fund account");
    String::new()
}

// ============ ОТПРАВКА ОРДЕРА ============
async fn place_sandbox_order(
    token: &str,
    figi: &str,
    direction: &str,
    price: f64,
    quantity: i64,
    account_id: &str,
    risk_manager: &mut RiskManager,
) {
    if account_id.is_empty() {
        println!("⚠️ Cannot place order: no account ID");
        return;
    }

    let units = price.floor() as i64;
    let nano = ((price - units as f64) * 1_000_000_000.0) as i32;

    let url = "https://sandbox-invest-public-api.tbank.ru/rest/tinkoff.public.invest.api.contract.v1.OrdersService/PostOrder";

    let body = json!({
        "figi": figi,
        "quantity": quantity,
        "price": {
            "units": units,
            "nano": nano
        },
        "direction": direction,
        "accountId": account_id,
        "orderType": "ORDER_TYPE_MARKET"
    });

    let client = reqwest::Client::builder()
        .danger_accept_invalid_certs(true)
        .build()
        .unwrap();

    match client
        .post(url)
        .header("Authorization", format!("Bearer {}", token))
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await
    {
        Ok(resp) => {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            if status.is_success() {
                println!(
                    "✅ ORDER EXECUTED: {} {} @ {:.2}",
                    direction, quantity, price
                );
                TRADES_TOTAL.inc();
                if direction == "ORDER_DIRECTION_BUY" {
                    LONG_TRADES.inc();
                    let spent = quantity as f64 * price;
                    risk_manager.capital -= spent;
                    PORTFOLIO_VALUE.set(risk_manager.capital);
                } else {
                    SHORT_TRADES.inc();
                }
                risk_manager.save_state().await;
            } else {
                println!("❌ Order failed ({}): {}", status, text);
            }
        }
        Err(e) => println!("❌ Order error: {}", e),
    }
}

// ============ СИНХРОНИЗАЦИЯ С РЕАЛЬНЫМ ПОРТФЕЛЕМ ============
async fn sync_with_exchange(
    token: &str,
    account_id: &str,
    figi: &str,
    risk_manager: &mut RiskManager,
) -> Result<(), anyhow::Error> {
    let url = "https://sandbox-invest-public-api.tbank.ru/rest/tinkoff.public.invest.api.contract.v1.OperationsService/GetPositions";

    let client = reqwest::Client::builder()
        .danger_accept_invalid_certs(true)
        .build()?;

    let body = json!({
        "accountId": account_id
    });

    let resp = client
        .post(url)
        .header("Authorization", format!("Bearer {}", token))
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await?;

    if resp.status().is_success() {
        let data: serde_json::Value = resp.json().await?;

        if let Some(money) = data["money"].as_array() {
            for m in money {
                if m["currency"].as_str() == Some("rub") {
                    let units = m["units"]
                        .as_str()
                        .unwrap_or("0")
                        .parse::<f64>()
                        .unwrap_or(0.0);
                    let nano = m["nano"].as_i64().unwrap_or(0) as f64 / 1_000_000_000.0;
                    risk_manager.capital = units + nano;
                    PORTFOLIO_VALUE.set(risk_manager.capital);
                    break;
                }
            }
        }

        if let Some(securities) = data["securities"].as_array() {
            for sec in securities {
                if sec["figi"].as_str() == Some(figi) {
                    if let Some(quantity_str) = sec["balance"].as_str() {
                        let quantity = quantity_str.parse::<i64>().unwrap_or(0);
                        if quantity > 0 && risk_manager.get_active_position().is_none() {
                            let price_url = "https://sandbox-invest-public-api.tbank.ru/rest/tinkoff.public.invest.api.contract.v1.MarketDataService/GetOrderBook";
                            let price_body = json!({ "figi": figi, "depth": 1 });

                            let price_resp = client
                                .post(price_url)
                                .header("Authorization", format!("Bearer {}", token))
                                .header("Content-Type", "application/json")
                                .json(&price_body)
                                .send()
                                .await?;

                            let current_price = if price_resp.status().is_success() {
                                let price_data: serde_json::Value = price_resp.json().await?;
                                if let Some(bids) = price_data["bids"].as_array() {
                                    if let Some(bid) = bids.first() {
                                        let units = bid["price"]["units"]
                                            .as_str()
                                            .unwrap_or("0")
                                            .parse::<i64>()
                                            .unwrap_or(0);
                                        let nano = bid["price"]["nano"].as_i64().unwrap_or(0);
                                        units as f64 + nano as f64 / 1_000_000_000.0
                                    } else {
                                        322.50
                                    }
                                } else {
                                    322.50
                                }
                            } else {
                                322.50
                            };

                            println!(
                                "📊 Found position: {} shares at current price {:.2}",
                                quantity, current_price
                            );

                            let atr = 3.14;
                            risk_manager.active_position = Some(Position {
                                side: "LONG".to_string(),
                                entry_price: current_price,
                                quantity,
                                stop_loss: current_price - atr * 1.5,
                                take_profit: current_price + atr * 3.0,
                            });
                            CURRENT_POSITION_SIZE.set(quantity as f64);
                            TRADES_TOTAL.inc();
                            LONG_TRADES.inc();
                            risk_manager.save_state().await;
                            println!("✅ Synced position from exchange");
                        }
                    }
                }
            }
        }
    }
    Ok(())
}

// ============ MAIN ============
#[tokio::main]
async fn main() {
    let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();

    let token = std::env::var("TINVEST_TOKEN").expect("TINVEST_TOKEN not set");
    let figi = "BBG004730N88";
    let initial_capital = 100000.0;

    let client = Client::builder()
        .danger_accept_invalid_certs(true)
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .unwrap();

    let account_id = create_and_fund_account(&token).await;
    if account_id.is_empty() {
        println!("❌ FATAL: No account ID available");
        return;
    }

    let state_manager = StateManager::new("redis://localhost:6379", figi);
    let saved_state = state_manager.load_state().await;

    let mut risk_manager = if let Some(state) = saved_state {
        println!("📀 Loaded previous state from Redis");
        let mut rm = RiskManager::new(state.capital, figi, Some(state_manager.clone()));
        rm.daily_pnl = state.daily_pnl;
        if let Some(pos) = state.active_position {
            rm.active_position = Some(Position {
                side: pos.side,
                entry_price: pos.entry_price,
                quantity: pos.quantity,
                stop_loss: pos.stop_loss,
                take_profit: pos.take_profit,
            });
            println!(
                "📊 Restored position: {} shares at {:.2}",
                pos.quantity, pos.entry_price
            );
            CURRENT_POSITION_SIZE.set(pos.quantity as f64);
            PORTFOLIO_VALUE.set(rm.capital);
        }
        rm
    } else {
        println!("📀 No saved state, starting fresh");
        RiskManager::new(initial_capital, figi, Some(state_manager.clone()))
    };

    if let Err(e) = sync_with_exchange(&token, &account_id, figi, &mut risk_manager).await {
        println!("⚠️ Sync error: {}", e);
    }

    tokio::spawn(metrics_server());
    println!("📊 Metrics server running on http://localhost:8080/metrics");

    println!("🚀 T-Invest Three Factor Bot Started");
    println!("📈 Instrument: {}", figi);
    println!("💰 Account ID: {}", account_id);
    println!("💵 Initial Capital: {:.0} RUB", initial_capital);
    println!("📉 Daily Loss Limit: 3%");
    println!("📊 Position Size: 5-30% of capital (aggressive)");
    println!("⏰ Update interval: 5 minutes");
    println!("{}", "=".repeat(60));

    loop {
        let start_time = Utc::now();
        risk_manager.reset_daily_pnl();

        if risk_manager.get_active_position().is_some() {
            let orderbook_url = "https://sandbox-invest-public-api.tbank.ru/rest/tinkoff.public.invest.api.contract.v1.MarketDataService/GetOrderBook";
            let orderbook_body = json!({ "figi": figi, "depth": 1 });

            if let Ok(orderbook_resp) = client
                .post(orderbook_url)
                .header("Authorization", format!("Bearer {}", token))
                .header("Content-Type", "application/json")
                .json(&orderbook_body)
                .send()
                .await
            {
                if orderbook_resp.status().is_success() {
                    if let Ok(data) = orderbook_resp.json::<serde_json::Value>().await {
                        if let Some(bids) = data["bids"].as_array() {
                            if let Some(bid) = bids.first() {
                                let price_units = bid["price"]["units"]
                                    .as_str()
                                    .unwrap_or("0")
                                    .parse::<i64>()
                                    .unwrap_or(0);
                                let price_nano = bid["price"]["nano"].as_i64().unwrap_or(0);
                                let current_price =
                                    price_units as f64 + price_nano as f64 / 1_000_000_000.0;
                                risk_manager
                                    .check_stop_loss_take_profit(current_price)
                                    .await;
                            }
                        }
                    }
                }
            }
        }

        if let Err(e) = sync_with_exchange(&token, &account_id, figi, &mut risk_manager).await {
            println!("⚠️ Sync error: {}", e);
        }

        if !risk_manager.check_daily_limit() {
            println!("⏸️ Trading paused for today due to loss limit");
            time::sleep(time::Duration::from_secs(300)).await;
            continue;
        }

        if risk_manager.get_active_position().is_some() {
            println!("\n📊 Position already open, monitoring...");
            time::sleep(time::Duration::from_secs(300)).await;
            continue;
        }

        let candles_url = "https://sandbox-invest-public-api.tbank.ru/rest/tinkoff.public.invest.api.contract.v1.MarketDataService/GetCandles";
        let from = start_time - Duration::days(30);
        let candles_body = json!({
            "figi": figi,
            "from": from.to_rfc3339(),
            "to": start_time.to_rfc3339(),
            "interval": 5
        });

        let mut candles = Vec::new();
        if let Ok(candles_resp) = client
            .post(candles_url)
            .header("Authorization", format!("Bearer {}", token))
            .header("Content-Type", "application/json")
            .json(&candles_body)
            .send()
            .await
        {
            if candles_resp.status().is_success() {
                if let Ok(data) = candles_resp.json::<serde_json::Value>().await {
                    if let Some(candles_data) = data["candles"].as_array() {
                        for cd in candles_data {
                            if let Ok(c) = Candle::from_tinvest_json(figi.to_string(), cd) {
                                if c.is_complete {
                                    save_candle_to_clickhouse(figi, &c, &client).await;
                                    candles.push(c);
                                }
                            }
                        }
                    }
                }
            }
        }

        let orderbook_url = "https://sandbox-invest-public-api.tbank.ru/rest/tinkoff.public.invest.api.contract.v1.MarketDataService/GetOrderBook";
        let orderbook_body = json!({ "figi": figi, "depth": 10 });

        let mut orderbook = OrderBook::new(figi.to_string());
        if let Ok(orderbook_resp) = client
            .post(orderbook_url)
            .header("Authorization", format!("Bearer {}", token))
            .header("Content-Type", "application/json")
            .json(&orderbook_body)
            .send()
            .await
        {
            if orderbook_resp.status().is_success() {
                if let Ok(data) = orderbook_resp.json::<serde_json::Value>().await {
                    if let Some(bids) = data["bids"].as_array() {
                        for bid in bids {
                            let price_units = bid["price"]["units"]
                                .as_str()
                                .unwrap_or("0")
                                .parse::<i64>()
                                .unwrap_or(0);
                            let price_nano = bid["price"]["nano"].as_i64().unwrap_or(0);
                            let price = Price::from_float(
                                price_units as f64 + price_nano as f64 / 1_000_000_000.0,
                            );
                            let volume = bid["quantity"]
                                .as_str()
                                .unwrap_or("0")
                                .parse::<u64>()
                                .unwrap_or(0);
                            orderbook.update_bid(price, volume);
                        }
                    }
                    if let Some(asks) = data["asks"].as_array() {
                        for ask in asks {
                            let price_units = ask["price"]["units"]
                                .as_str()
                                .unwrap_or("0")
                                .parse::<i64>()
                                .unwrap_or(0);
                            let price_nano = ask["price"]["nano"].as_i64().unwrap_or(0);
                            let price = Price::from_float(
                                price_units as f64 + price_nano as f64 / 1_000_000_000.0,
                            );
                            let volume = ask["quantity"]
                                .as_str()
                                .unwrap_or("0")
                                .parse::<u64>()
                                .unwrap_or(0);
                            orderbook.update_ask(price, volume);
                        }
                    }
                }
            }
        }

        save_orderbook_to_clickhouse(figi, &orderbook, &client).await;

        let analyzer = Analyzer::new(14);
        let atr = if candles.len() > 0 {
            analyzer.calculate_atr(&candles)
        } else {
            0.0
        };
        let trend = if candles.len() > 0 {
            Analyzer::trend(&candles, 14)
        } else {
            0
        };
        let last = candles.last();
        let candle_signal = if let Some(c) = last {
            Analyzer::candle_signal(c)
        } else {
            0
        };
        let pressure = orderbook.pressure();
        let current_price = last.map(|c| c.close.as_float()).unwrap_or(0.0);

        let sentiment_factor = get_aggregated_sentiment(&client).await;
        let bayesian_factor = get_bayesian(
            current_price,
            atr,
            trend,
            pressure,
            sentiment_factor,
            &client,
        )
        .await;
        let lgbm_factor = get_lgbm(
            current_price,
            atr,
            trend,
            pressure,
            sentiment_factor,
            &client,
        )
        .await;

        let ml_factor = bayesian_factor * 0.5 + lgbm_factor * 0.5;
        let weighted_sum = sentiment_factor * 0.6 + ml_factor * 0.4;
        let main_filter = pressure < 0.0;
        let confidence = (weighted_sum + 1.0) / 2.0;

        PORTFOLIO_VALUE.set(risk_manager.capital);
        DAILY_PNL.set(risk_manager.daily_pnl);
        SIGNAL_STRENGTH.observe(confidence);
        PRESSURE_VALUE.set(pressure);
        SENTIMENT_VALUE.set(sentiment_factor);

        if let Ok(mut redis) = RedisClient::connect().await {
            let decision = if main_filter && weighted_sum > 0.3 {
                "LONG"
            } else if main_filter && weighted_sum < -0.3 {
                "SHORT"
            } else {
                "WAIT"
            };
            let _ = redis
                .publish_signal(decision, current_price, pressure)
                .await;
        }

        println!("\n[{}]", start_time.format("%Y-%m-%d %H:%M:%S"));
        println!("Price: {:.2} | ATR: {:.2}", current_price, atr);
        println!(
            "Trend: {} | Candle: {}",
            match trend {
                1 => "UP",
                -1 => "DOWN",
                _ => "→",
            },
            match candle_signal {
                2 => "🔥BUY",
                1 => "↑",
                -1 => "↓",
                -2 => "🔥SELL",
                _ => "●",
            }
        );
        println!(
            "Pressure: {:.4} ({})",
            pressure,
            if pressure < 0.0 {
                "BULLISH"
            } else if pressure > 0.0 {
                "BEARISH"
            } else {
                "NEUTRAL"
            }
        );
        println!(
            "Sentiment: {:.2} | Bayesian: {:.2} | LightGBM: {:.2} | ML avg: {:.2}",
            sentiment_factor, bayesian_factor, lgbm_factor, ml_factor
        );
        println!(
            "Weighted sum: {:.2} | Confidence: {:.2} | Daily PnL: {:.2} RUB",
            weighted_sum, confidence, risk_manager.daily_pnl
        );

        if main_filter && weighted_sum > 0.3 {
            println!("🔥🔥 SIGNAL: LONG ENTRY");
            let quantity = risk_manager.calculate_position_size(confidence, atr, current_price);
            if quantity > 0 {
                if risk_manager
                    .open_position("LONG".to_string(), current_price, quantity, atr, confidence)
                    .await
                {
                    place_sandbox_order(
                        &token,
                        figi,
                        "ORDER_DIRECTION_BUY",
                        current_price,
                        quantity,
                        &account_id,
                        &mut risk_manager,
                    )
                    .await;
                }
            }
        } else if main_filter && weighted_sum < -0.3 {
            println!("❄️❄️ SIGNAL: SHORT ENTRY");
            let quantity = risk_manager.calculate_position_size(confidence, atr, current_price);
            if quantity > 0 {
                if risk_manager
                    .open_position(
                        "SHORT".to_string(),
                        current_price,
                        quantity,
                        atr,
                        confidence,
                    )
                    .await
                {
                    place_sandbox_order(
                        &token,
                        figi,
                        "ORDER_DIRECTION_SELL",
                        current_price,
                        quantity,
                        &account_id,
                        &mut risk_manager,
                    )
                    .await;
                }
            }
        } else {
            println!("⏸️ SIGNAL: WAIT");
            if !main_filter {
                println!("   Reason: Main filter FALSE (pressure >= 0)");
            } else {
                println!(
                    "   Reason: Weighted sum {:.2} not exceeding threshold",
                    weighted_sum
                );
            }
        }

        PORTFOLIO_VALUE.set(risk_manager.capital);
        println!("\n💤 Sleeping for 5 minutes...");
        time::sleep(time::Duration::from_secs(300)).await;
    }
}
