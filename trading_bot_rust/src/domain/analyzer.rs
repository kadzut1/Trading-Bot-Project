use super::Candle;

#[derive(Debug, Clone)]
pub struct Analyzer {
    window: usize,
}

impl Analyzer {
    pub fn new(window: usize) -> Self {
        Self { window }
    }

    pub fn calculate_atr(&self, candles: &[Candle]) -> f64 {
        if candles.len() < 2 {
            return 0.0;
        }
        let mut tr_sum = 0.0;
        for i in 1..candles.len().min(self.window + 1) {
            let high = candles[i].high.as_float();
            let low = candles[i].low.as_float();
            let prev_close = candles[i - 1].close.as_float();
            let tr = (high - low)
                .abs()
                .max((high - prev_close).abs())
                .max((low - prev_close).abs());
            tr_sum += tr;
        }
        tr_sum / candles.len().min(self.window) as f64
    }

    pub fn candle_signal(candle: &Candle) -> i8 {
        let body = (candle.close.as_float() - candle.open.as_float()).abs();
        let range = candle.high.as_float() - candle.low.as_float();
        if range == 0.0 {
            return 0;
        }
        let body_ratio = body / range;

        if candle.close.as_float() > candle.open.as_float() {
            if body_ratio > 0.6 {
                2
            } else if body_ratio > 0.3 {
                1
            } else {
                0
            }
        } else if candle.close.as_float() < candle.open.as_float() {
            if body_ratio > 0.6 {
                -2
            } else if body_ratio > 0.3 {
                -1
            } else {
                0
            }
        } else {
            0
        }
    }

    pub fn trend(candles: &[Candle], period: usize) -> i8 {
        if candles.len() < period + 5 {
            return 0;
        }
        let mut ma_sum = 0.0;
        for i in candles.len() - period..candles.len() {
            ma_sum += candles[i].close.as_float();
        }
        let ma_current = ma_sum / period as f64;

        let mut ma_prev_sum = 0.0;
        for i in candles.len() - period * 2..candles.len() - period {
            ma_prev_sum += candles[i].close.as_float();
        }
        let ma_prev = ma_prev_sum / period as f64;

        if ma_current > ma_prev * 1.005 {
            1
        } else if ma_current < ma_prev * 0.995 {
            -1
        } else {
            0
        }
    }
}
