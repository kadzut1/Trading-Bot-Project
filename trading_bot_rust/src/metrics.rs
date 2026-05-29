use lazy_static::lazy_static;
use prometheus::{register_counter, register_gauge, register_histogram, Counter, Gauge, Histogram};

lazy_static! {
    pub static ref TRADES_TOTAL: Counter =
        register_counter!("trades_total", "Total number of trades executed").unwrap();
    pub static ref LONG_TRADES: Counter =
        register_counter!("long_trades_total", "Total number of long trades").unwrap();
    pub static ref SHORT_TRADES: Counter =
        register_counter!("short_trades_total", "Total number of short trades").unwrap();
    pub static ref PORTFOLIO_VALUE: Gauge =
        register_gauge!("portfolio_value", "Current portfolio value in RUB").unwrap();
    pub static ref DAILY_PNL: Gauge =
        register_gauge!("daily_pnl", "Daily profit/loss in RUB").unwrap();
    pub static ref SIGNAL_STRENGTH: Histogram = register_histogram!(
        "signal_strength",
        "Strength of trading signals",
        vec![0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8, 0.9, 1.0]
    )
    .unwrap();
    pub static ref PRESSURE_VALUE: Gauge =
        register_gauge!("orderbook_pressure", "Current orderbook pressure").unwrap();
    pub static ref SENTIMENT_VALUE: Gauge =
        register_gauge!("news_sentiment", "Current news sentiment").unwrap();
    pub static ref API_LATENCY: Histogram = register_histogram!(
        "api_latency_seconds",
        "API request latency in seconds",
        vec![0.01, 0.05, 0.1, 0.5, 1.0, 2.0, 5.0]
    )
    .unwrap();
}

pub fn init_metrics() {
    // Регистрируем метрики
    let _ = &*TRADES_TOTAL;
    let _ = &*LONG_TRADES;
    let _ = &*SHORT_TRADES;
    let _ = &*PORTFOLIO_VALUE;
    let _ = &*DAILY_PNL;
    let _ = &*SIGNAL_STRENGTH;
    let _ = &*PRESSURE_VALUE;
    let _ = &*SENTIMENT_VALUE;
    let _ = &*API_LATENCY;
}
